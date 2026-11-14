use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Instant;

use replicant_game::sky::{
    RecordAssembler, SkyBroadcast, SkyHold, merge_record, read_asset_names, read_sky,
    read_table_record, split_record, write_asset_pointers, write_table_record,
    write_weather_counters,
};
use replicant_game::{PhaseEntry, Replicant};
use replicant_proto::{Message, SkyAssets};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net;
use crate::sky::calls::SkyCalls;

static SYNC: LazyLock<Mutex<SkySync>> = LazyLock::new(|| Mutex::new(SkySync::default()));
static LEAKED_NAMES: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Mutex::default);
static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

#[derive(Default)]
struct SkySync {
    broadcast: SkyBroadcast,
    hold: SkyHold,
    assembler: RecordAssembler,
    was_host: Option<bool>,
}

pub fn tick_sky_sync(build: &VerifiedBuild) {
    let mut sync = SYNC.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(net) = net::handle() else {
        return;
    };
    let incoming = net.take_relayed(|message| {
        matches!(
            message,
            Message::Sky(_) | Message::SkyRecord(_) | Message::SkyAssets(_)
        )
    });
    let view = net.view();
    let Some(remote) = view.partner_state().cloned() else {
        sync.broadcast.reset();
        sync.hold.reset();
        sync.assembler = RecordAssembler::default();
        sync.was_host = None;
        return;
    };
    let is_host = remote.slot == 1;
    if sync.was_host != Some(is_host) {
        sync.broadcast.reset();
        sync.hold.reset();
        sync.was_host = Some(is_host);
    }
    let game = Replicant::new(InProcessMemory::new(build));
    let Some(phase) = game
        .current_phase()
        .ok()
        .and_then(|name| PhaseEntry::index_of(&name))
        .and_then(|index| u16::try_from(index).ok())
    else {
        return;
    };
    let Ok(reading) = read_sky(game.memory()) else {
        return;
    };
    let now_ms = u64::try_from(EPOCH.elapsed().as_millis()).unwrap_or(u64::MAX);
    if is_host {
        let guest_phase = Some(remote.state.phase);
        if let Some(send) = sync.broadcast.next(now_ms, phase, &reading, guest_phase) {
            log_line(&format!(
                "sky: host sends ({}) phase {} env {} blend {:.2} wind {} weather {}/{}",
                send.reason,
                send.state.phase,
                send.state.env_area,
                send.state.env_blend_seconds,
                send.state.wind_area,
                send.state.weather,
                send.state.weather_offset,
            ));
            net.send_relayed(Message::Sky(send.state));
            if let Ok(record) = read_table_record(game.memory(), send.state.env_area) {
                let chunks = split_record(phase, send.state.env_area, &record);
                log_line(&format!(
                    "sky: host sends record of area {} in {} chunks",
                    send.state.env_area,
                    chunks.len()
                ));
                for chunk in chunks {
                    net.send_relayed(Message::SkyRecord(chunk));
                }
            }
            if let Ok(names) = read_asset_names(game.memory(), send.state.env_area) {
                log_line(&format!(
                    "sky: host sends assets of area {} {names:?}",
                    send.state.env_area
                ));
                net.send_relayed(Message::SkyAssets(SkyAssets {
                    phase,
                    area: send.state.env_area,
                    names,
                }));
            }
        }
        return;
    }
    for message in incoming {
        if let Message::SkyRecord(chunk) = &message
            && let Some(host_record) = sync.assembler.receive(chunk)
            && chunk.phase == phase
        {
            apply_host_record(build, game.memory(), chunk.area, &host_record);
        }
        if let Message::SkyAssets(assets) = &message
            && assets.phase == phase
        {
            apply_host_assets(build, assets);
        }
        if let Message::Sky(state) = message {
            log_line(&format!(
                "sky: guest got phase {} env {} wind {} weather {}/{} (guest phase {phase}, guest env {})",
                state.phase,
                state.env_area,
                state.wind_area,
                state.weather,
                state.weather_offset,
                reading.env_area,
            ));
            sync.hold.receive(state);
        }
    }
    let apply = sync.hold.step(now_ms, phase, &reading);
    if apply.is_empty() {
        return;
    }
    let calls = SkyCalls::new(build);
    if let Some((weather, offset)) = apply.weather {
        let mut memory = InProcessMemory::new(build);
        let result = write_weather_counters(&mut memory, weather, offset);
        log_line(&format!(
            "sky: guest sets weather {weather}/{offset} (was {}/{}, ok {})",
            reading.weather,
            reading.weather_offset,
            result.is_ok()
        ));
    }
    if let Some((area, blend)) = apply.env {
        calls.set_environment_area(area, blend);
        log_line(&format!(
            "sky: guest sets env {area} blend {blend:.2} (was {})",
            reading.env_area
        ));
    }
    if let Some((area, blend)) = apply.wind {
        calls.set_wind_area(area, blend);
        log_line(&format!(
            "sky: guest sets wind {area} blend {blend:.2} (was {})",
            reading.wind_area
        ));
    }
}

fn apply_host_record(
    build: &VerifiedBuild,
    memory: &InProcessMemory,
    area: i32,
    host_record: &[u8],
) {
    let Ok(local) = read_table_record(memory, area) else {
        log_line(&format!(
            "sky: guest cannot read its table record of area {area}"
        ));
        return;
    };
    let merged = merge_record(host_record, &local);
    if merged == local {
        return;
    }
    let mut writer = InProcessMemory::new(build);
    let written = write_table_record(&mut writer, area, &merged).is_ok();
    log_line(&format!(
        "sky: guest record of area {area} written {written}"
    ));
    if written {
        SkyCalls::new(build).set_environment_area(area, 0.0);
    }
}

fn apply_host_assets(build: &VerifiedBuild, assets: &SkyAssets) {
    let pointers: Vec<u64> = assets
        .names
        .iter()
        .map(|name| leaked_name_pointer(name))
        .collect();
    let mut writer = InProcessMemory::new(build);
    let written = write_asset_pointers(&mut writer, assets.area, &pointers).is_ok();
    log_line(&format!(
        "sky: guest sets assets of area {} {:?} written {written}",
        assets.area, assets.names
    ));
    if written {
        SkyCalls::new(build).set_environment_area(assets.area, 0.0);
    }
}

fn leaked_name_pointer(name: &str) -> u64 {
    if name.is_empty() {
        return 0;
    }
    let mut names = LEAKED_NAMES.lock().unwrap_or_else(PoisonError::into_inner);
    *names.entry(name.to_owned()).or_insert_with(|| {
        let mut bytes = name.as_bytes().to_vec();
        bytes.push(0);
        Box::leak(bytes.into_boxed_slice()).as_ptr() as usize as u64
    })
}
