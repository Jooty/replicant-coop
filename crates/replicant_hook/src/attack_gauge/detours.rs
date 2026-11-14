use core::ffi::c_void;
use std::panic;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use replicant_game::GameMemory;
use replicant_game::attack_gauge::attack_gauge_boot_text;
use replicant_game::attack_gauge::{
    ATTACK_GAUGE_DUMMY_HIT_RVA, ATTACK_GAUGE_NODE_HP_OFFSET, ATTACK_GAUGE_REMAINING_SECONDS_OFFSET,
    GUEST_GAUGE_MEASURING_HP, GaugeNodeReading, GaugeSwingMiss, UPDATE_ATTACK_GAUGE_RVA,
    guest_node_hp_after_own_hit, guest_remaining_floor, measured_gauge_damage, own_gauge_hit_text,
    read_gauge, write_node_hp,
};
use replicant_game::enemies::AttackRecord;
use replicant_proto::{AttackGaugeHit, Message};

use crate::attack_gauge::state::{
    GUEST, GUEST_HOLDS_GAUGE_HITS, HOOKED_BUILD, HeldGaugeHit, ORIGINAL_DUMMY_HIT, ORIGINAL_UPDATE,
    now_ms, off_file_present,
};
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net;

type DummyHit = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, i32) -> u64;
type Update = unsafe extern "system" fn(*mut c_void, f32);

struct Measuring {
    serial: Option<u32>,
    nodes: Vec<(u8, GaugeNodeReading, i32)>,
}

pub fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    log_line(attack_gauge_boot_text(off_file_present()));
    #[expect(
        unsafe_code,
        reason = "hooks two attack gauge functions by their addresses"
    )]
    // SAFETY: the RVAs start the dummy's hit function (dummy, record, result, collision index;
    // CActorCADummy vslot 0x558) and the per-frame update (manager, seconds in xmm1) in the
    // verified build; each replacement has the same signature and calls the original.
    unsafe {
        hook_address(
            "attack gauge dummy hit",
            (build.module_base() + u64::from(ATTACK_GAUGE_DUMMY_HIT_RVA)) as usize,
            dummy_hit_detour as *const () as *mut c_void,
            &ORIGINAL_DUMMY_HIT,
        );
        hook_address(
            "attack gauge update",
            (build.module_base() + u64::from(UPDATE_ATTACK_GAUGE_RVA)) as usize,
            update_detour as *const () as *mut c_void,
            &ORIGINAL_UPDATE,
        );
    }
}

#[expect(
    unsafe_code,
    reason = "calls the original dummy hit function through the trampoline"
)]
extern "system" fn dummy_hit_detour(
    dummy: *mut c_void,
    record: *mut c_void,
    result: *mut c_void,
    collision: i32,
) -> u64 {
    let measuring = if GUEST_HOLDS_GAUGE_HITS.load(Ordering::Acquire) {
        panic::catch_unwind(|| begin_measuring(dummy as u64))
            .ok()
            .flatten()
    } else {
        None
    };
    let original = ORIGINAL_DUMMY_HIT.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    // SAFETY: the trampoline continues into the original body, which has this exact signature.
    let outcome = unsafe {
        core::mem::transmute::<*mut c_void, DummyHit>(original)(dummy, record, result, collision)
    };
    if let Some(measuring) = measuring {
        let _ = panic::catch_unwind(|| finish_measuring(measuring, record as u64));
    }
    outcome
}

#[expect(
    unsafe_code,
    reason = "calls the original gauge update through the trampoline"
)]
extern "system" fn update_detour(manager: *mut c_void, seconds: f32) {
    if GUEST_HOLDS_GAUGE_HITS.load(Ordering::Acquire) {
        let _ = panic::catch_unwind(|| hold_clock(manager as u64));
    }
    let original = ORIGINAL_UPDATE.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    // SAFETY: the trampoline continues into the original body, which has this exact signature.
    unsafe { core::mem::transmute::<*mut c_void, Update>(original)(manager, seconds) };
}

fn hold_clock(manager: u64) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let mut memory = InProcessMemory::new(build);
    let address = manager + ATTACK_GAUGE_REMAINING_SECONDS_OFFSET;
    let Ok(remaining) = memory.read_f32(address) else {
        return;
    };
    if let Some(floor) = guest_remaining_floor(remaining) {
        let _ = memory.write_f32(address, floor);
    }
}

fn begin_measuring(dummy: u64) -> Option<Measuring> {
    let build = HOOKED_BUILD.get()?;
    let mut guest = GUEST.lock().unwrap_or_else(PoisonError::into_inner);
    let mut memory = InProcessMemory::new(build);
    let reading = read_gauge(&memory)?;
    if reading.dummy != dummy {
        guest.tally.miss(GaugeSwingMiss::OtherDummy);
        return None;
    }
    let mut nodes = Vec::new();
    for (index, node) in (0_u8..).zip(reading.nodes) {
        // A node at 0 is broken; the hit function skips it, as on the host.
        if node.node.hp >= 1 && write_node_hp(&mut memory, &node, GUEST_GAUGE_MEASURING_HP) {
            nodes.push((index, node, node.node.hp));
        }
    }
    Some(Measuring {
        serial: guest.mirrored_serial,
        nodes,
    })
}

fn finish_measuring(measuring: Measuring, record: u64) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let mut memory = InProcessMemory::new(build);
    let hit_id = AttackRecord::read(&memory, record).map_or(0, |attack| attack.hit_id);
    let mut took_any = false;
    for (index, node, shown) in measuring.nodes {
        let after = memory
            .read_i32(node.address + ATTACK_GAUGE_NODE_HP_OFFSET)
            .unwrap_or(GUEST_GAUGE_MEASURING_HP);
        let damage = measured_gauge_damage(GUEST_GAUGE_MEASURING_HP, after);
        took_any |= damage.is_some();
        let counted =
            damage.is_some_and(|damage| count_hit(measuring.serial, index, hit_id, damage));
        let hp = match damage {
            Some(damage) if counted => guest_node_hp_after_own_hit(shown, damage),
            _ => shown,
        };
        let _ = write_node_hp(&mut memory, &node, hp);
    }
    if !took_any {
        GUEST
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tally
            .miss(GaugeSwingMiss::GameTookNothing);
    }
}

fn count_hit(serial: Option<u32>, node: u8, hit_id: u32, damage: i32) -> bool {
    let mut guest = GUEST.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(seq) = guest.hits.take(now_ms(), node, hit_id) else {
        guest.tally.miss(GaugeSwingMiss::SameSwing);
        drop(guest);
        log_line(&format!(
            "attack gauge: the same swing hit node {node} again, not sent"
        ));
        return false;
    };
    let Some(serial) = serial else {
        guest.held_hits.push(HeldGaugeHit { node, damage, seq });
        guest.tally.held = guest.tally.held.saturating_add(1);
        drop(guest);
        log_line(&format!(
            "attack gauge: your hit on node {node} took {damage}, held as number {seq} until the host's opening arrives"
        ));
        return true;
    };
    let phase = guest.phase;
    let sent = send_gauge_hit(phase, serial, HeldGaugeHit { node, damage, seq });
    if sent {
        guest.tally.sent = guest.tally.sent.saturating_add(1);
    } else {
        guest.tally.miss(GaugeSwingMiss::NotConnected);
    }
    sent
}

pub(super) fn send_gauge_hit(phase: u16, serial: u32, hit: HeldGaugeHit) -> bool {
    let Some(net) = net::handle() else {
        return false;
    };
    net.send_relayed(Message::AttackGaugeHit(AttackGaugeHit {
        phase,
        serial,
        node: hit.node,
        damage: hit.damage,
        seq: hit.seq,
    }));
    log_line(&own_gauge_hit_text(hit.node, hit.damage, hit.seq, serial));
    true
}
