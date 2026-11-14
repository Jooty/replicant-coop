use replicant_game::prompt_follow::{
    PARTNER_CLOSE_COUNTS_IF_SEEN_BEFORE_THIS_COPY_OPENED_WITHIN_MS, PROMPT_PAGE_SHOWN_DIALOG_STATE,
    PromptClose, PromptDismissal, PromptFollow, PromptPageId, PromptView, ScriptDialogNative,
    format_prompt_unattended_close,
};

const OPENING: i32 = 0x2;
const CLOSING: i32 = 0x4;
const SHOWN: i32 = PROMPT_PAGE_SHOWN_DIALOG_STATE;
const INFO_PROMPT_KIND_OF_THE_RUN: i32 = 2036;
const RELAYED_HANDLER_ARRIVED_ON_THE_HOST_MS: u64 = 26_387;
const HOSTS_COPY_SHOWN_MS: u64 = 27_393;
const GUEST_CLOSED_ITS_OWN_MS: u64 = 27_768;

fn info_page() -> PromptPageId {
    PromptPageId {
        native: Some(ScriptDialogNative::Start2DInfo),
        kind: INFO_PROMPT_KIND_OF_THE_RUN,
        id: 0,
        page: 1,
    }
}

fn outside(dialog_state: i32, now_ms: u64, window_unattended: bool) -> PromptView {
    PromptView {
        dialog_state,
        now_ms,
        phase: Some(7),
        partner_in_same_phase: true,
        window_unattended,
        ..PromptView::default()
    }
}

fn show(
    follow: &mut PromptFollow,
    native: ScriptDialogNative,
    led_since_ms: Option<u64>,
    shown_ms: u64,
    window_unattended: bool,
) {
    follow.tick(outside(0, shown_ms - 2, window_unattended));
    follow.note_dialog_open_in(Some(native), INFO_PROMPT_KIND_OF_THE_RUN, 0, led_since_ms);
    follow.tick(outside(OPENING, shown_ms - 1, window_unattended));
    let step = follow.tick(outside(SHOWN, shown_ms, window_unattended));
    assert_eq!(step.close, None);
    assert_eq!(step.unattended_close, None);
}

fn guest_dismissed(follow: &mut PromptFollow, page: PromptPageId, times: u32, received_ms: u64) {
    follow.receive_guest_dismissal_at(
        PromptDismissal {
            page,
            times,
            in_fight: false,
        },
        received_ms,
    );
}

#[test]
fn the_hosts_copy_of_an_info_prompt_the_guest_led_closes_when_the_guest_closes_its_own() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        Some(RELAYED_HANDLER_ARRIVED_ON_THE_HOST_MS),
        HOSTS_COPY_SHOWN_MS,
        true,
    );
    guest_dismissed(&mut follow, info_page(), 1, GUEST_CLOSED_ITS_OWN_MS);
    let step = follow.tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, true));
    assert_eq!(
        step.close,
        Some(PromptClose {
            page: info_page(),
            try_number: 1
        })
    );
}

#[test]
fn a_guest_led_info_prompt_closes_with_the_guest_even_while_the_host_is_at_its_window() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        Some(RELAYED_HANDLER_ARRIVED_ON_THE_HOST_MS),
        HOSTS_COPY_SHOWN_MS,
        false,
    );
    guest_dismissed(&mut follow, info_page(), 1, GUEST_CLOSED_ITS_OWN_MS);
    let step = follow.tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, false));
    assert!(step.close.is_some());
}

#[test]
fn a_guest_close_that_arrived_before_the_hosts_copy_opened_still_closes_it() {
    let mut follow = PromptFollow::new();
    follow.tick(outside(0, 0, true));
    guest_dismissed(
        &mut follow,
        info_page(),
        1,
        RELAYED_HANDLER_ARRIVED_ON_THE_HOST_MS + 400,
    );
    follow.note_dialog_open_in(
        Some(ScriptDialogNative::Start2DInfo),
        INFO_PROMPT_KIND_OF_THE_RUN,
        0,
        Some(RELAYED_HANDLER_ARRIVED_ON_THE_HOST_MS),
    );
    follow.tick(outside(OPENING, HOSTS_COPY_SHOWN_MS - 1, true));
    let step = follow.tick(outside(SHOWN, HOSTS_COPY_SHOWN_MS, true));
    assert_eq!(step.close.map(|close| close.page), Some(info_page()));
}

#[test]
fn a_guest_count_does_not_close_the_hosts_own_info_prompt_its_player_is_reading() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        None,
        HOSTS_COPY_SHOWN_MS,
        false,
    );
    guest_dismissed(&mut follow, info_page(), 1, GUEST_CLOSED_ITS_OWN_MS);
    let step = follow.tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, false));
    assert_eq!(step.close, None);
    assert_eq!(step.unattended_close, None);
}

#[test]
fn an_own_info_prompt_nobody_is_reading_closes_when_the_partner_closes_the_same_page() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        None,
        HOSTS_COPY_SHOWN_MS,
        true,
    );
    guest_dismissed(&mut follow, info_page(), 1, GUEST_CLOSED_ITS_OWN_MS);
    let step = follow.tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, true));
    assert_eq!(step.close, None);
    assert_eq!(
        step.unattended_close,
        Some(PromptClose {
            page: info_page(),
            try_number: 1
        })
    );
}

#[test]
fn an_own_info_prompt_left_open_closes_once_its_player_walks_away_from_the_window() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        None,
        HOSTS_COPY_SHOWN_MS,
        false,
    );
    guest_dismissed(&mut follow, info_page(), 1, GUEST_CLOSED_ITS_OWN_MS);
    assert_eq!(
        follow
            .tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, false))
            .unattended_close,
        None
    );
    let step = follow.tick(outside(SHOWN, 60_000, true));
    assert!(step.unattended_close.is_some());
}

#[test]
fn an_unattended_own_prompt_is_not_closed_by_a_partner_count_from_well_before_it_opened() {
    let mut follow = PromptFollow::new();
    follow.tick(outside(0, 0, true));
    guest_dismissed(&mut follow, info_page(), 1, 1_000);
    let shown_ms = 1_000 + PARTNER_CLOSE_COUNTS_IF_SEEN_BEFORE_THIS_COPY_OPENED_WITHIN_MS + 1;
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        None,
        shown_ms,
        true,
    );
    let step = follow.tick(outside(SHOWN, shown_ms + 500, true));
    assert_eq!(step.unattended_close, None);
}

#[test]
fn an_unattended_own_prompt_reopened_just_after_is_not_closed_again_by_the_same_count() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        None,
        HOSTS_COPY_SHOWN_MS,
        true,
    );
    guest_dismissed(&mut follow, info_page(), 1, GUEST_CLOSED_ITS_OWN_MS);
    let step = follow.tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, true));
    assert!(step.unattended_close.is_some());
    follow.tick(outside(CLOSING, GUEST_CLOSED_ITS_OWN_MS + 32, true));
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        None,
        GUEST_CLOSED_ITS_OWN_MS + 500,
        true,
    );
    let step = follow.tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 600, true));
    assert_eq!(step.unattended_close, None);
}

#[test]
fn an_unattended_own_money_or_word_window_stays_its_players_own() {
    for native in [
        ScriptDialogNative::StartInfoMoney,
        ScriptDialogNative::StartInfoWord,
    ] {
        let mut follow = PromptFollow::new();
        show(&mut follow, native, None, HOSTS_COPY_SHOWN_MS, true);
        let page = PromptPageId {
            native: Some(native),
            ..info_page()
        };
        guest_dismissed(&mut follow, page, 1, GUEST_CLOSED_ITS_OWN_MS);
        let step = follow.tick(outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, true));
        assert_eq!(step.unattended_close, None, "{native:?}");
        assert_eq!(step.close, None, "{native:?}");
    }
}

#[test]
fn an_unattended_own_prompt_needs_the_partner_in_the_same_phase() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::Start2DInfo,
        None,
        HOSTS_COPY_SHOWN_MS,
        true,
    );
    guest_dismissed(&mut follow, info_page(), 1, GUEST_CLOSED_ITS_OWN_MS);
    let elsewhere = PromptView {
        partner_in_same_phase: false,
        ..outside(SHOWN, GUEST_CLOSED_ITS_OWN_MS + 16, true)
    };
    assert_eq!(follow.tick(elsewhere).unattended_close, None);
}

#[test]
fn a_guest_count_still_closes_the_hosts_own_reward_prompt_as_before() {
    let mut follow = PromptFollow::new();
    show(
        &mut follow,
        ScriptDialogNative::StartInfoItem,
        None,
        1_000,
        false,
    );
    let page = PromptPageId {
        native: Some(ScriptDialogNative::StartInfoItem),
        ..info_page()
    };
    guest_dismissed(&mut follow, page, 1, 1_700);
    let step = follow.tick(outside(SHOWN, 2_600, false));
    assert_eq!(step.close.map(|close| close.page), Some(page));
}

#[test]
fn the_unattended_close_line_says_why() {
    assert_eq!(
        format_prompt_unattended_close(PromptClose {
            page: info_page(),
            try_number: 1
        }),
        "prompt follow: closing your _Start2DInfo 2036/0 page 1, your partner dismissed theirs and nobody is at this window"
    );
}
