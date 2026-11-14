use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicPtr, AtomicU8, Ordering};
use std::sync::{Mutex, PoisonError};

use replicant_game::GameMemory;
use replicant_game::marker::UI_DIALOG_STATE_RVA;
use replicant_game::prompt_follow::{
    PromptDismissal, PromptFollow, PromptPageId, PromptView,
    SCRIPT_DIALOG_OPENED_BY_THE_TUTORIAL_OPENER_WIRE_CODE, ScriptDialogNative,
    UI_DIALOG_OPEN_FUNCTION_RVA, dialog_wait_parked, format_guest_dismissal, format_host_dismissal,
    format_prompt_catch_up_close, format_prompt_catch_up_gave_up, format_prompt_follow_close,
    format_prompt_follow_gave_up, format_prompt_opened, format_prompt_unattended_close,
};
use replicant_game::stay_together::FollowRole;
use replicant_game::tutorial_prompt::OPEN_TUTORIAL_FUNCTION_RVA;
use replicant_proto::{FightPromptCount, Message};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::fight_sync::radio_waits::{call_original, wait_state};
use crate::fight_sync::state::{BUILD, GUEST, Here, covers_phase};
use crate::game_hooks::{
    game_window_is_foreground, hook_address, host_led_run, host_led_runs_enter_phase,
    note_yielding_native, slot_holder_name, window_unfocused,
};
use crate::memory::InProcessMemory;
use crate::net::{self, net_clock_ms};
use crate::stay_together::{close_script_prompt_page, pause_menu_open};

static PROMPTS: Mutex<PromptFollow> = Mutex::new(PromptFollow::new());
static LAST_PHASE: Mutex<u16> = Mutex::new(0);
static OPENS_NOT_TAKEN_YET: Mutex<Vec<OpenedDialog>> = Mutex::new(Vec::new());
static ORIGINAL_OPEN_PROMPT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_UI_DIALOG_OPEN: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_DIALOG_POLLS: [AtomicPtr<c_void>; 6] =
    [const { AtomicPtr::new(ptr::null_mut()) }; 6];
static DIALOG_POLL_RUNNING_WIRE_CODE: AtomicU8 =
    AtomicU8::new(SCRIPT_DIALOG_OPENED_BY_THE_TUTORIAL_OPENER_WIRE_CODE);
static DIALOG_ID_OPENED_IN_THIS_POLL: Mutex<Option<i32>> = Mutex::new(None);
static DIALOG_WAIT_STILL_WAITING_AT: Mutex<Option<(ScriptDialogNative, u64)>> = Mutex::new(None);

struct OpenedDialog {
    native: Option<ScriptDialogNative>,
    kind: i32,
    id: i32,
    host_led: Option<(String, u64)>,
}

type OpenPrompt = unsafe extern "system" fn(*mut c_void, i32, i32);
type UiDialogOpen = unsafe extern "system" fn(*mut c_void, i32, u8, u8, u8) -> u8;

pub(super) fn install(build: VerifiedBuild) {
    let base = build.module_base();
    #[expect(unsafe_code, reason = "hooks the script prompt openers by address")]
    // SAFETY: the RVAs start UiDialog::OpenTutorial (void; rcx dialog, edx kind, r8d id) and
    // UiDialog::Open (al opened; rcx dialog, edx dialog id, r8b, r9b, stack byte force) in the
    // verified build, per their prologues and docs symbols.tsv; the replacements match them.
    unsafe {
        hook_address(
            "UiDialog::OpenTutorial",
            (base + u64::from(OPEN_TUTORIAL_FUNCTION_RVA)) as usize,
            open_prompt_detour as *const () as *mut c_void,
            &ORIGINAL_OPEN_PROMPT,
        );
        hook_address(
            "UiDialog::Open",
            (base + u64::from(UI_DIALOG_OPEN_FUNCTION_RVA)) as usize,
            ui_dialog_open_detour as *const () as *mut c_void,
            &ORIGINAL_UI_DIALOG_OPEN,
        );
    }
    let detours: [*mut c_void; 6] = [
        start_2d_info_poll_detour as *const () as *mut c_void,
        start_info_item_poll_detour as *const () as *mut c_void,
        start_info_money_poll_detour as *const () as *mut c_void,
        start_info_quest_poll_detour as *const () as *mut c_void,
        start_info_word_poll_detour as *const () as *mut c_void,
        start_info_tutorial_poll_detour as *const () as *mut c_void,
    ];
    for ((native, detour), original) in ScriptDialogNative::ALL
        .into_iter()
        .zip(detours)
        .zip(&ORIGINAL_DIALOG_POLLS)
    {
        #[expect(
            unsafe_code,
            reason = "hooks the polled waits of script natives by address"
        )]
        // SAFETY: each RVA starts the wait the script step polls for that native in the verified
        // build (0x707370, 0x7083D0, 0x708490, 0x708530, 0x708670, 0x7085D0; docs symbols.tsv):
        // the script manager + 0xD8 in, a byte out (non-zero while it waits).
        unsafe {
            hook_address(
                &format!("LuaNative:{}", native.name()),
                (base + u64::from(native.poll_rva())) as usize,
                detour,
                original,
            );
        }
    }
}

#[expect(
    unsafe_code,
    reason = "calls the original opener through the trampoline MinHook built"
)]
extern "system" fn open_prompt_detour(dialog: *mut c_void, kind: i32, id: i32) {
    let original = ORIGINAL_OPEN_PROMPT.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    // SAFETY: the trampoline continues into the original opener, which has this exact signature.
    unsafe { core::mem::transmute::<*mut c_void, OpenPrompt>(original)(dialog, kind, id) };
    let _ = panic::catch_unwind(|| note_open_if_it_opened(kind, id));
}

#[expect(
    unsafe_code,
    reason = "calls the original dialog open through the trampoline MinHook built"
)]
extern "system" fn ui_dialog_open_detour(
    dialog: *mut c_void,
    dialog_id: i32,
    first_flag: u8,
    second_flag: u8,
    force: u8,
) -> u8 {
    let original = ORIGINAL_UI_DIALOG_OPEN.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    // SAFETY: the trampoline continues into the original open, which has this exact signature.
    let opened = unsafe {
        core::mem::transmute::<*mut c_void, UiDialogOpen>(original)(
            dialog,
            dialog_id,
            first_flag,
            second_flag,
            force,
        )
    };
    let _ = panic::catch_unwind(|| note_dialog_id_if_a_poll_opened_it(opened != 0, dialog_id));
    opened
}

extern "system" fn start_2d_info_poll_detour(args: *mut c_void) -> u8 {
    dialog_poll(0, args)
}

extern "system" fn start_info_item_poll_detour(args: *mut c_void) -> u8 {
    dialog_poll(1, args)
}

extern "system" fn start_info_money_poll_detour(args: *mut c_void) -> u8 {
    dialog_poll(2, args)
}

extern "system" fn start_info_quest_poll_detour(args: *mut c_void) -> u8 {
    dialog_poll(3, args)
}

extern "system" fn start_info_word_poll_detour(args: *mut c_void) -> u8 {
    dialog_poll(4, args)
}

extern "system" fn start_info_tutorial_poll_detour(args: *mut c_void) -> u8 {
    dialog_poll(5, args)
}

fn dialog_poll(index: usize, args: *mut c_void) -> u8 {
    let native = ScriptDialogNative::ALL[index];
    // The wait state dword is 0 only on the call that opens the dialog.
    let starts = panic::catch_unwind(|| wait_state(args as u64) == Some(0)).unwrap_or(false);
    DIALOG_POLL_RUNNING_WIRE_CODE.store(native.wire_code(), Ordering::Relaxed);
    let still_waiting = call_original(&ORIGINAL_DIALOG_POLLS[index], args);
    DIALOG_POLL_RUNNING_WIRE_CODE.store(
        SCRIPT_DIALOG_OPENED_BY_THE_TUTORIAL_OPENER_WIRE_CODE,
        Ordering::Relaxed,
    );
    let _ = panic::catch_unwind(|| dialog_poll_returned(native, starts, still_waiting != 0));
    still_waiting
}

fn dialog_poll_returned(native: ScriptDialogNative, starts: bool, still_waiting: bool) {
    note_yielding_native(native.name());
    let now_ms = net_clock_ms();
    *DIALOG_WAIT_STILL_WAITING_AT
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = still_waiting.then_some((native, now_ms));
    let dialog_id = DIALOG_ID_OPENED_IN_THIS_POLL
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if !(starts && still_waiting && native.opens_through_its_poll()) {
        return;
    }
    // The word window opens through its own opener, which passes no dialog id.
    OPENS_NOT_TAKEN_YET
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(OpenedDialog {
            native: Some(native),
            kind: dialog_id.unwrap_or(0),
            id: 0,
            host_led: host_led_run(),
        });
}

fn running_dialog_poll() -> Option<ScriptDialogNative> {
    ScriptDialogNative::from_wire_code(DIALOG_POLL_RUNNING_WIRE_CODE.load(Ordering::Relaxed))
}

fn note_dialog_id_if_a_poll_opened_it(opened: bool, dialog_id: i32) {
    let Some(native) = running_dialog_poll() else {
        return;
    };
    if !opened || !native.opens_through_its_poll() {
        return;
    }
    *DIALOG_ID_OPENED_IN_THIS_POLL
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(dialog_id);
}

fn note_open_if_it_opened(kind: i32, id: i32) {
    let Some(build) = BUILD.get() else {
        return;
    };
    let state = InProcessMemory::new(build)
        .read_i32(build.module_base() + u64::from(UI_DIALOG_STATE_RVA))
        .unwrap_or(0);
    if state == 0 {
        return;
    }
    let native = running_dialog_poll().filter(|native| !native.opens_through_its_poll());
    OPENS_NOT_TAKEN_YET
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(OpenedDialog {
            native,
            kind,
            id,
            host_led: host_led_run(),
        });
}

fn parked_dialog_wait(now_ms: u64) -> Option<ScriptDialogNative> {
    let parked = *DIALOG_WAIT_STILL_WAITING_AT
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    parked
        .filter(|(_, polled_ms)| dialog_wait_parked(Some(*polled_ms), now_ms))
        .map(|(native, _)| native)
}

pub(super) fn tick(build: &VerifiedBuild, here: Option<Here>, counts: &[FightPromptCount]) {
    let local_phase = here.and_then(|here| here.local_phase);
    let role = here.map(|here| here.role);
    let partner_in_same_phase = here.is_some_and(|here| here.partner_in_same_phase);
    let stream_active = role.is_some() && local_phase.is_some_and(covers_phase);
    if let Some(phase) = local_phase.filter(|_| stream_active) {
        *LAST_PHASE.lock().unwrap_or_else(PoisonError::into_inner) = phase;
    }
    let now_ms = net_clock_ms();
    host_led_runs_enter_phase(local_phase);
    let dialog_state = InProcessMemory::new(build)
        .read_i32(build.module_base() + u64::from(UI_DIALOG_STATE_RVA))
        .unwrap_or(0);
    let opens = std::mem::take(
        &mut *OPENS_NOT_TAKEN_YET
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
    );
    let parked_in = parked_dialog_wait(now_ms);
    let entry_behind_ms = if role == Some(FollowRole::Guest) {
        GUEST
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry_behind_ms(local_phase, now_ms)
    } else {
        None
    };
    let mut prompts = PROMPTS.lock().unwrap_or_else(PoisonError::into_inner);
    for open in opens {
        let since_ms = open.host_led.as_ref().map(|(_, since_ms)| *since_ms);
        prompts.note_dialog_open_in(open.native, open.kind, open.id, since_ms);
        let handler = open.host_led.as_ref().map(|(name, _)| name.as_str());
        log_line(&format_prompt_opened(
            open.native,
            open.kind,
            open.id,
            handler,
        ));
    }
    if let Some(role) = role {
        receive_partner_counts(&mut prompts, counts, local_phase, now_ms, role);
    }
    let step = prompts.tick(PromptView {
        stream_active,
        dialog_state,
        pause_menu_open: pause_menu_open(build),
        now_ms,
        phase: local_phase,
        partner_in_same_phase,
        parked_in_dialog_wait: parked_in.is_some(),
        entry_behind_ms,
        window_unattended: window_unfocused() || !game_window_is_foreground(),
    });
    drop(prompts);
    if let Some(role) = role
        && let Some(dismissal) = step.dismissed
        && (role == FollowRole::Host || guest_shares_dismissal(dismissal))
    {
        send_dismissal(dismissal, local_phase);
    }
    if role.is_none() {
        return;
    }
    if let Some(close) = step.close {
        log_line(&format_prompt_follow_close(close));
        close_script_prompt_page(build);
    }
    if let Some(close) = step.unattended_close {
        log_line(&format_prompt_unattended_close(close));
        close_script_prompt_page(build);
    }
    if let Some(page) = step.gave_up_on {
        log_line(&format_prompt_follow_gave_up(page));
    }
    if role != Some(FollowRole::Guest) {
        return;
    }
    let holder = || slot_holder_name().unwrap_or_else(|| "an unknown handler".to_owned());
    if let Some(close) = step.catch_up_close {
        log_line(&format_prompt_catch_up_close(close, parked_in, &holder()));
        close_script_prompt_page(build);
    }
    if step.catch_up_gave_up {
        log_line(&format_prompt_catch_up_gave_up(&holder()));
    }
}

fn guest_shares_dismissal(dismissal: PromptDismissal) -> bool {
    !dismissal.in_fight
}

fn receive_partner_counts(
    prompts: &mut PromptFollow,
    counts: &[FightPromptCount],
    local_phase: Option<u16>,
    now_ms: u64,
    role: FollowRole,
) {
    for count in counts
        .iter()
        .filter(|count| Some(count.phase) == local_phase)
    {
        let native = ScriptDialogNative::from_wire_code(count.native);
        if native.is_none() && count.native != SCRIPT_DIALOG_OPENED_BY_THE_TUTORIAL_OPENER_WIRE_CODE
        {
            continue;
        }
        let from_guest = role == FollowRole::Host;
        if from_guest && count.in_fight {
            continue;
        }
        let dismissal = PromptDismissal {
            page: PromptPageId {
                native,
                kind: count.kind,
                id: count.id,
                page: count.page,
            },
            times: count.dismissed,
            in_fight: count.in_fight,
        };
        if from_guest {
            prompts.receive_guest_dismissal_at(dismissal, now_ms);
        } else {
            prompts.receive_host_dismissal_at(dismissal, now_ms);
        }
        log_line(&if from_guest {
            format_guest_dismissal(dismissal)
        } else {
            format_host_dismissal(dismissal)
        });
    }
}

fn send_dismissal(dismissal: PromptDismissal, local_phase: Option<u16>) {
    let phase = if dismissal.in_fight {
        Some(*LAST_PHASE.lock().unwrap_or_else(PoisonError::into_inner))
    } else {
        local_phase
    };
    let (Some(phase), Some(net)) = (phase, net::handle()) else {
        return;
    };
    net.send_relayed(Message::FightPromptCount(FightPromptCount {
        phase,
        kind: dismissal.page.kind,
        id: dismissal.page.id,
        page: dismissal.page.page,
        dismissed: dismissal.times,
        native: dismissal.page.wire_native_code(),
        in_fight: dismissal.in_fight,
    }));
}
