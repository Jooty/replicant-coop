use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, ColorImage, UiBuilder, vec2};
use egui_kittest::Harness;
use egui_kittest::wgpu::WgpuTestRenderer;
use nc_launcher::app::{
    ConsoleRun, JoinDialog, Launcher, NetForms, Screen, Tab, UpdateState, apply_launcher_look,
    copy_line_id, draw, draw_parts_sheet, fixture_campaign_card, fixture_launcher,
};
use nc_launcher::home::GameCard;
use nc_launcher::install::ModStatus;
use nc_launcher::net::{
    ConnectionReport, ConnectionState, LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, LauncherNet,
    LinkStatus, LobbyReady, MemberStatus, NetError, NetState, NetStatus, SessionOptions,
};
use nc_launcher::saves::{SlotSummary, save_dir};
use nc_launcher::steam::GameInstall;
use nc_launcher_core::{
    ALL_PROFILES, ConsoleClock, ConsoleTask, GameProfile, INVALID_TICKET_TEXT, LauncherScale,
    REPLICANT, SessionEndCause, banner_facts, judge_rejection_text,
};
use nc_net::{
    AddressKind, AddressReach, IrohConfig, IrohTransport, LOCAL_LOOKUP_PORT, LinkProgress,
    LookupMeans, PathRoute, ProgressEntry, Transport,
};
use replicant_proto::{CampaignCard, PartnerBody, RejectReason, SaveSummary, SecondSeatCard};

const WINDOW_SIZE_PX: (u32, u32) = (1180, 760);
const SNAPSHOT_LOOKUP_PORT: u16 = 47_591;
const MINIMUM_WINDOW_SIZE_PX: (u32, u32) = (900, 600);
const NARROW_WINDOW_SIZE_PX: (u32, u32) = (900, 760);
const GENERATED_ART_SIZE_PX: (usize, usize) = (1920, 620);
const GENERATED_SUBJECT_SIZE_PX: (usize, usize) = (360, 420);
const PARTS_SHEET_FRAMES: usize = 3;
const SNAPSHOT_FOLDER: &str = "launcher-snapshots";
const SCRATCH_FOLDER: &str = "test-scratch/launcher-snapshots";
const FIXTURE_TICKET: &str = "rcn2ticketfixtureaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FIXTURE_READY_BUILD: &str = "7b04de2";
const FIXTURE_BUILD_SERVER: &str = "builds.example:7878";
const FIXTURE_PLAYER_NAME: &str = "Jay";
const FIXTURE_PARTNER_PING_MS: u32 = 38;
const TICKET_COPY_SALT: &str = "lobby-ticket";
const FIXTURE_CAMPAIGN_ID: &str = "c0ffee00c0ffee00";
const FIXTURE_LOG_LINES: [&str; 7] = [
    "launcher 0.1.0 started",
    "found the game folder",
    "mod installed, build 3f2a91c",
    "hosting a lobby",
    "Mira connected",
    "NieR Replicant: the game connected to the launcher",
    "the game started and linked to the launcher; the partner's game is loading the co-op save in slot 3, which can take a while on a cold disk",
];
const GUEST_NAMES: [&str; 1] = ["Mira"];
const MOTION_FOLDER: &str = "motion";
const MOTION_FRAME_MS: f32 = 1000.0 / 60.0;
const MOTION_SETTLE_MAX_STEPS: u64 = 240;
const MOTION_SETTLED_STEPS: u64 = 1;
const COPY_HOLD_FRAMES_SECONDS: f32 = 1.6;
const DRIFT_FRAME_MS: f32 = 50.0;
const DRIFT_FRAMES: usize = 40;
const HOME_ROW_ART_POINT: egui::Pos2 = egui::pos2(900.0, 250.0);
const HOME_AWAY_POINT: egui::Pos2 = egui::pos2(150.0, 740.0);
const HOME_UPDATE_LINE_POINT: egui::Pos2 = egui::pos2(220.0, 286.0);
const CONSOLE_START_FRAMES: usize = 7;
const CONSOLE_JOIN_MIDWAY_FRAMES: usize = 90;
const CONSOLE_REFUSAL_FRAME: usize = 56;
const CONSOLE_TICKET_FRAME: usize = 18;
const CONSOLE_DONE_FRAMES: usize = 78;
const CONSOLE_CLIP_FRAMES: usize = 130;
const CONSOLE_CLIP_WORK_DONE_FRAME: usize = 45;
const FIXTURE_PICKED_SLOT: u8 = 2;
const FIXTURE_RELAY: &str = "euc1-1";
const FIXTURE_RELAY_ROUND_TRIP_MS: u64 = 62;
const HOME_ROW_RIGHT_CLICK_POINT: egui::Pos2 = egui::pos2(560.0, 180.0);
const MENU_ENTRY_OFFSET: egui::Vec2 = egui::vec2(60.0, 24.0);
const FIXTURE_STEAM_ACCOUNT_FOLDER: &str = "Steam/76561190000000000";
const ENDED_BAR_RIGHTMOST_BUTTON_POINT: egui::Pos2 = egui::pos2(1120.0, 720.0);
const HOME_FIRST_COMMAND_POINT: egui::Pos2 = egui::pos2(110.0, 366.0);
const SHORT_GAME_FOLDER: &str = "../../target/tmp/hs/G";

struct FixtureNet {
    state: NetState,
    progress: Vec<ProgressEntry>,
}

impl LauncherNet for FixtureNet {
    fn host(&self) -> Result<String, NetError> {
        Err(NetError::Unavailable)
    }

    fn join(&self, _ticket: &str) -> Result<(), NetError> {
        Err(NetError::Unavailable)
    }

    fn test_connection(&self, _ticket: &str, _seconds: u32) -> Result<ConnectionReport, NetError> {
        Err(NetError::Unavailable)
    }

    fn leave(&self) {}

    fn progress(&self) -> Vec<ProgressEntry> {
        self.progress.clone()
    }

    fn status(&self) -> NetStatus {
        NetStatus {
            state: self.state.clone(),
        }
    }
}

struct Page {
    tab: Tab,
    launcher: Launcher,
    forms: NetForms,
    ticket_copied_once: bool,
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(SCRATCH_FOLDER)
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

fn found_card(profile: &'static GameProfile, root: &Path) -> GameCard {
    let game_dir = root.join(profile.install_folder_name);
    std::fs::create_dir_all(&game_dir).expect("game folder");
    std::fs::write(game_dir.join(profile.exe_names[0]), b"").expect("fake exe");
    let dll = game_dir.join(profile.hook_dll_name);
    std::fs::write(&dll, b"").expect("fake dll");
    GameCard {
        profile,
        install: Some(GameInstall {
            profile,
            game_dir,
            steamapps_dir: root.to_path_buf(),
            proton_prefix: None,
        }),
        dll_source: Some(dll),
        mod_status: ModStatus::Current,
        installed_build: Some("3f2a91c".to_owned()),
        ready_build: None,
        exe_supported: true,
    }
}

fn every_game_found(root: &Path) -> Vec<GameCard> {
    ALL_PROFILES
        .iter()
        .map(|profile| found_card(profile, root))
        .collect()
}

fn generated_art(profile: &GameProfile) -> ColorImage {
    let (width, height) = GENERATED_ART_SIZE_PX;
    let focus = banner_facts(profile.game_id).focus;
    let subject_x = (focus.x * width as f32) as usize;
    let subject_y = (focus.y * height as f32) as usize;
    let (subject_width, subject_height) = GENERATED_SUBJECT_SIZE_PX;
    let in_subject = |x: usize, y: usize| {
        x + subject_width / 2 >= subject_x
            && x < subject_x + subject_width / 2
            && y + subject_height / 2 >= subject_y
            && y < subject_y + subject_height / 2
    };
    let pixels = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            if in_subject(x, y) {
                let stripe = ((x / 24 + y / 24) % 2) as u8;
                return Color32::from_rgb(200 + 30 * stripe, 190 + 30 * stripe, 170);
            }
            let across = x as f32 / width as f32;
            let down = y as f32 / height as f32;
            let block = ((x / 160 + y / 120) % 3) as f32 * 10.0;
            Color32::from_rgb(
                (60.0 + 90.0 * across + block) as u8,
                (70.0 + 60.0 * down + block) as u8,
                (110.0 + 80.0 * (1.0 - across) + block) as u8,
            )
        })
        .collect();
    ColorImage::new([width, height], pixels)
}

fn with_generated_art(mut launcher: Launcher) -> Launcher {
    for profile in ALL_PROFILES {
        launcher
            .banner_art
            .insert_image(profile.game_id, Some(generated_art(profile)));
    }
    launcher
}

fn without_art(mut launcher: Launcher) -> Launcher {
    for profile in ALL_PROFILES {
        launcher.banner_art.insert_image(profile.game_id, None);
    }
    launcher
}

fn lobby_link(guests: usize, as_guest: bool, max_players: u8) -> LinkStatus {
    let mut link = LinkStatus {
        state: ConnectionState::Connected,
        ticket: (!as_guest).then(|| FIXTURE_TICKET.to_owned()),
        game_linked: false,
        guest: as_guest,
        link_port: 7000,
        ..LinkStatus::default()
    };
    link.rules.max_players = max_players;
    link.own_slot = Some(if as_guest { guests as u8 } else { 0 });
    link.members = (0..guests)
        .map(|index| {
            MemberStatus::new(
                index as u8 + u8::from(!as_guest),
                GUEST_NAMES[index % GUEST_NAMES.len()].to_owned(),
            )
        })
        .collect();
    link
}

fn net_state_for(link: &LinkStatus, hosting_alone: bool) -> NetState {
    if hosting_alone {
        return NetState::Hosting {
            ticket: FIXTURE_TICKET.to_owned(),
        };
    }
    NetState::Connected {
        peer_name: link
            .members
            .first()
            .map_or_else(String::new, |member| member.name.clone()),
    }
}

fn game_page(launcher: Launcher, state: NetState) -> Page {
    let mut forms = NetForms::new(Arc::new(FixtureNet {
        state,
        progress: Vec::new(),
    }));
    forms.session_options = SessionOptions::default();
    Page {
        tab: Tab::NewGame,
        launcher,
        forms,
        ticket_copied_once: false,
    }
}

fn render_page(name: &str, page: Page) {
    render_page_at(name, page, WINDOW_SIZE_PX);
}

fn render_page_at(name: &str, page: Page, size: (u32, u32)) {
    render_page_with_pointer(name, page, size, None);
}

fn render_page_with_pointer(
    name: &str,
    mut page: Page,
    (width, height): (u32, u32),
    pointer: Option<egui::Pos2>,
) {
    page.launcher.new_game_name = FIXTURE_PLAYER_NAME.to_owned();
    let mut harness = Harness::builder()
        .with_size(vec2(width as f32, height as f32))
        .with_accessibility_check(false)
        .renderer(WgpuTestRenderer::new())
        .with_context_setup(apply_launcher_look)
        .build_ui_state(draw_page, page);
    harness.run();
    if let Some(pointer) = pointer {
        harness.hover_at(pointer);
        harness.run();
    }
    let image = harness.render().expect("off-screen render");
    assert_eq!((image.width(), image.height()), (width, height));
    save_snapshot(name, &image);
}

fn draw_page(ui: &mut egui::Ui, page: &mut Page) {
    // The harness wraps the page in an 8 px margin; draw over the whole window instead.
    let window = ui.ctx().content_rect();
    if page.ticket_copied_once {
        page.ticket_copied_once = false;
        let now = ui.input(|input| input.time);
        ui.data_mut(|data| data.insert_temp(copy_line_id(TICKET_COPY_SALT), now));
    }
    ui.scope_builder(UiBuilder::new().max_rect(window), |ui| {
        draw(ui, &mut page.tab, &mut page.launcher, &mut page.forms);
    });
}

fn save_snapshot(name: &str, image: &image::RgbaImage) {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(SNAPSHOT_FOLDER);
    std::fs::create_dir_all(&folder).expect("snapshot folder");
    image
        .save(folder.join(format!("{name}.png")))
        .expect("png written");
}

fn home_launcher(root: &Path) -> Launcher {
    let mut cards = every_game_found(root);
    for card in &mut cards {
        if card.profile.game_id == REPLICANT.game_id {
            card.mod_status = ModStatus::Outdated;
            card.ready_build = Some(FIXTURE_READY_BUILD.to_owned());
        }
    }
    fixture_launcher(&REPLICANT, cards, LinkStatus::default(), Screen::Home)
}

#[test]
fn home_with_the_game_found() {
    let root = scratch_dir("home");
    let launcher = with_generated_art(home_launcher(&root));
    render_page("home-game-found", game_page(launcher, NetState::Idle));
}

#[test]
fn home_with_an_update_waiting_and_running() {
    let root = scratch_dir("home-update");
    let launcher = with_generated_art(home_launcher(&root));
    render_page_with_pointer(
        "home-update-pointed",
        game_page(launcher, NetState::Idle),
        WINDOW_SIZE_PX,
        Some(HOME_UPDATE_LINE_POINT),
    );
    let root = scratch_dir("home-update-running");
    let mut launcher = with_generated_art(home_launcher(&root));
    launcher.updater.state = UpdateState::Updating;
    render_page("home-update-running", game_page(launcher, NetState::Idle));
}

#[test]
fn home_with_the_game_missing() {
    let root = scratch_dir("home-missing");
    let mut launcher = with_generated_art(home_launcher(&root));
    for card in &mut launcher.cards {
        card.install = None;
        card.mod_status = ModStatus::NotInstalled;
        card.installed_build = None;
        card.ready_build = None;
    }
    render_page("home-game-missing", game_page(launcher, NetState::Idle));
}

#[test]
fn home_without_art() {
    let root = scratch_dir("home-no-art");
    let launcher = without_art(home_launcher(&root));
    render_page("home-without-art", game_page(launcher, NetState::Idle));
}

#[test]
fn home_at_900_wide() {
    let root = scratch_dir("home-narrow");
    let launcher = with_generated_art(home_launcher(&root));
    render_page_at(
        "home-900-wide",
        game_page(launcher, NetState::Idle),
        NARROW_WINDOW_SIZE_PX,
    );
}

#[test]
fn home_at_the_minimum_window_size() {
    let root = scratch_dir("home-minimum");
    let launcher = with_generated_art(home_launcher(&root));
    render_page_at(
        "home-minimum-size",
        game_page(launcher, NetState::Idle),
        MINIMUM_WINDOW_SIZE_PX,
    );
}

fn render_home_at_both_scales(name: &str, launcher: Launcher) {
    for (suffix, scale) in [
        ("", LauncherScale::Large),
        ("-medium", LauncherScale::Medium),
    ] {
        let page = game_page(launcher_copy(&launcher), NetState::Idle);
        let mut harness = stepped_harness_scaled(page, MOTION_FRAME_MS, WINDOW_SIZE_PX, scale);
        harness.run();
        save_snapshot(
            &format!("{name}{suffix}"),
            &harness.render().expect("off-screen render"),
        );
    }
}

fn launcher_copy(launcher: &Launcher) -> Launcher {
    let mut copy = with_generated_art(fixture_launcher(
        launcher.profile,
        launcher.cards.clone(),
        LinkStatus::default(),
        Screen::Home,
    ));
    copy.screen = launcher.screen;
    copy
}

fn without_an_update(launcher: &mut Launcher) {
    for card in &mut launcher.cards {
        card.mod_status = ModStatus::Current;
        card.ready_build = None;
    }
}

fn in_a_short_folder(launcher: &mut Launcher) {
    let game_dir = Path::new(SHORT_GAME_FOLDER);
    let _ = std::fs::remove_dir_all(game_dir);
    std::fs::create_dir_all(game_dir).expect("short game folder");
    for card in &mut launcher.cards {
        std::fs::write(game_dir.join(card.profile.exe_names[0]), b"").expect("fake exe");
        if let Some(install) = card.install.as_mut() {
            install.game_dir = game_dir.to_path_buf();
        }
    }
}

#[test]
fn home_with_a_long_folder_and_an_update() {
    let root = scratch_dir("home-long-update");
    render_home_at_both_scales("home-long-folder-update", home_launcher(&root));
}

#[test]
fn home_with_a_short_folder_and_no_update() {
    let root = scratch_dir("home-short-no-update");
    let mut launcher = home_launcher(&root);
    without_an_update(&mut launcher);
    in_a_short_folder(&mut launcher);
    render_home_at_both_scales("home-short-folder-no-update", launcher);
}

#[test]
fn home_with_an_exe_of_another_build() {
    let root = scratch_dir("home-other-build");
    let mut launcher = home_launcher(&root);
    without_an_update(&mut launcher);
    for card in &mut launcher.cards {
        card.exe_supported = false;
    }
    render_home_at_both_scales("home-exe-other-build", launcher);
}

#[test]
fn home_with_no_exe_in_the_folder() {
    let root = scratch_dir("home-no-exe");
    let mut launcher = home_launcher(&root);
    for card in &mut launcher.cards {
        if let Some(install) = &card.install {
            let _ = std::fs::remove_file(install.game_dir.join(card.profile.exe_names[0]));
        }
        card.exe_supported = false;
    }
    render_home_at_both_scales("home-no-exe", launcher);
}

#[test]
fn host_pressed_while_a_closed_lobby_lets_go_never_lands_on_settings() {
    let mut page = home_page("home-closing-lobby");
    page.launcher.link = hosting_alone_link();
    set_net_state(
        &mut page,
        NetState::Hosting {
            ticket: FIXTURE_TICKET.to_owned(),
        },
    );
    // Close lobby was just pressed; the network keeps reporting the lobby until it has let go of it.
    page.forms.leave();
    let mut harness = stepped_harness(page, MOTION_FRAME_MS);
    harness.run();
    save_snapshot(
        "home-while-the-lobby-closes",
        &harness.render().expect("off-screen render"),
    );
    click_at(&mut harness, HOME_FIRST_COMMAND_POINT);
    set_net_state(harness.state_mut(), NetState::Idle);
    harness.state_mut().launcher.link = LinkStatus::default();
    harness.run_steps(4);
    let page = harness.state();
    assert_ne!(
        (page.launcher.screen, page.tab),
        (Screen::Game, Tab::Settings)
    );
    assert!(page.forms.console.is_some(), "the press started a host");
}

fn ready_partner(link: &mut LinkStatus, ready: bool) {
    for member in &mut link.members {
        member.lobby = Some(LobbyReady {
            ready,
            mode: LOBBY_MODE_NEW_GAME,
            slot: None,
        });
        member.rtt_ms = Some(FIXTURE_PARTNER_PING_MS);
    }
}

fn lobby_page(name: &str, link: LinkStatus, state: NetState) -> Page {
    let root = scratch_dir(name);
    let launcher = with_generated_art(fixture_launcher(
        &REPLICANT,
        every_game_found(&root),
        link,
        Screen::Game,
    ));
    game_page(launcher, state)
}

fn hosting_alone_link() -> LinkStatus {
    LinkStatus {
        state: ConnectionState::Hosting,
        ticket: Some(FIXTURE_TICKET.to_owned()),
        own_slot: Some(0),
        ..LinkStatus::default()
    }
}

fn fixture_saves() -> Vec<SlotSummary> {
    vec![
        fixture_slot(0, "Tess", 8, 3.25),
        fixture_slot(1, "", 0, 0.0),
        fixture_slot(2, "Okabe", 31, 22.17),
        fixture_slot(3, "Rowan", 17, 11.5),
    ]
}

fn home_page(name: &str) -> Page {
    let root = scratch_dir(name);
    game_page(with_generated_art(home_launcher(&root)), NetState::Idle)
}

#[test]
fn home_with_a_lobby_open() {
    let mut page = home_page("home-lobby-open");
    page.launcher.link = hosting_alone_link();
    set_net_state(
        &mut page,
        NetState::Hosting {
            ticket: FIXTURE_TICKET.to_owned(),
        },
    );
    render_page_at("home-lobby-open", page, MINIMUM_WINDOW_SIZE_PX);
}

#[test]
fn home_lobby_failed() {
    let mut page = home_page("home-lobby-failed");
    let reason = SessionEndCause::HostLeft.reason_line().expect("a reason");
    set_net_state(&mut page, NetState::Failed(reason.to_owned()));
    page.launcher
        .session_ends
        .push((SessionEndCause::HostLeft, lobby_link(1, true, 2)));
    render_page("home-lobby-failed", page);
}

#[test]
fn an_end_from_the_other_side_off_the_lobby_page_leaves_the_player_there_with_the_reason_on_the_row()
 {
    let mut page = home_page("home-ended-elsewhere");
    set_net_state(&mut page, NetState::Failed(String::new()));
    page.launcher
        .session_ends
        .push((SessionEndCause::HostLeft, lobby_link(1, true, 2)));
    let mut harness = stepped_harness(page, MOTION_FRAME_MS);
    harness.run();
    let page = harness.state();
    assert_eq!(page.launcher.screen, Screen::Home);
    assert!(page.forms.ended_lobby.is_none());
    assert_eq!(
        page.forms
            .lobby_failure
            .as_ref()
            .map(|failure| failure.reason.as_str()),
        SessionEndCause::HostLeft.reason_line()
    );
    assert!(
        page.launcher
            .log
            .iter()
            .any(|line| line.ends_with("the launcher stayed where it was"))
    );
}

fn ended_lobby_page(name: &str, as_guest: bool) -> Page {
    let cause = if as_guest {
        SessionEndCause::HostLeft
    } else {
        SessionEndCause::LinkLost
    };
    let reason = cause.reason_line().expect("a reason").to_owned();
    let mut last_seen = lobby_link(1, as_guest, 2);
    ready_partner(&mut last_seen, true);
    let now = LinkStatus {
        state: ConnectionState::Failed(reason.clone()),
        guest: as_guest,
        ..LinkStatus::default()
    };
    let mut page = lobby_page(name, now, NetState::Failed(reason));
    if as_guest {
        page.forms.join_ticket = FIXTURE_TICKET.to_owned();
    }
    page.launcher.session_ends.push((cause, last_seen));
    page
}

fn render_ended_lobby(name: &str, as_guest: bool, scale: LauncherScale) {
    let mut harness = stepped_harness_scaled(
        ended_lobby_page(name, as_guest),
        MOTION_FRAME_MS,
        WINDOW_SIZE_PX,
        scale,
    );
    harness.run();
    assert!(harness.state().forms.ended_lobby.is_some());
    save_snapshot(name, &harness.render().expect("off-screen render"));
}

#[test]
fn the_guest_after_the_host_closed_the_lobby() {
    for (name, scale) in [
        ("lobby-ended-guest-home", LauncherScale::Large),
        ("lobby-ended-guest-home-medium", LauncherScale::Medium),
    ] {
        let mut harness = stepped_harness_scaled(
            ended_lobby_page(name, true),
            MOTION_FRAME_MS,
            WINDOW_SIZE_PX,
            scale,
        );
        harness.run();
        assert_eq!(harness.state().launcher.screen, Screen::Home);
        save_snapshot(name, &harness.render().expect("off-screen render"));
    }
}

#[test]
fn the_lobby_ended_by_the_network_as_the_host() {
    render_ended_lobby("lobby-ended-host", false, LauncherScale::Large);
    render_ended_lobby("lobby-ended-host-medium", false, LauncherScale::Medium);
}

#[test]
fn the_host_leaving_sends_the_guest_home_and_says_why_in_the_log() {
    let mut harness = stepped_harness(ended_lobby_page("lobby-ended-home", true), MOTION_FRAME_MS);
    harness.run();
    let page = harness.state();
    assert_eq!(page.launcher.screen, Screen::Home);
    assert!(page.forms.ended_lobby.is_none());
    assert_eq!(
        page.forms
            .lobby_failure
            .as_ref()
            .map(|failure| failure.reason.as_str()),
        SessionEndCause::HostLeft.reason_line()
    );
    assert_eq!(
        page.launcher.log.last().map(String::as_str),
        Some("session ended: the host left, from the other side; the launcher went home")
    );
}

#[test]
fn the_link_lost_keeps_the_host_on_the_lobby_page_ended() {
    let mut harness = stepped_harness(
        ended_lobby_page("lobby-ended-host-stays", false),
        MOTION_FRAME_MS,
    );
    harness.run();
    let page = harness.state();
    assert_eq!(page.launcher.screen, Screen::Game);
    assert_eq!(page.tab, Tab::NewGame);
    assert!(page.forms.ended_lobby.is_some());
}

fn click_at(harness: &mut Harness<'static, Page>, point: egui::Pos2) {
    harness.hover_at(point);
    harness.run_steps(2);
    harness.drag_at(point);
    harness.run_steps(2);
    harness.drop_at(point);
    harness.run_steps(2);
}

#[test]
fn home_on_an_ended_lobby_goes_home_and_keeps_the_reason_on_the_row() {
    let mut harness = stepped_harness(
        ended_lobby_page("lobby-ended-host-home", false),
        MOTION_FRAME_MS,
    );
    harness.run();
    click_at(&mut harness, ENDED_BAR_RIGHTMOST_BUTTON_POINT);
    let page = harness.state();
    assert_eq!(page.launcher.screen, Screen::Home);
    assert!(page.forms.ended_lobby.is_none());
    assert_eq!(
        page.forms
            .lobby_failure
            .as_ref()
            .map(|failure| failure.reason.as_str()),
        SessionEndCause::LinkLost.reason_line()
    );
}

fn join_dialog_page(name: &str, ticket: &str, refusal: Option<&'static str>) -> Page {
    let mut page = home_page(name);
    page.forms.join_ticket = ticket.to_owned();
    page.forms.join_dialog = Some(JoinDialog {
        refusal,
        focused_once: false,
    });
    page
}

#[test]
fn the_join_dialog() {
    render_page(
        "join-dialog-empty",
        join_dialog_page("join-empty", "", None),
    );
    render_page(
        "join-dialog-ticket",
        join_dialog_page("join-ticket", FIXTURE_TICKET, None),
    );
    render_page(
        "join-dialog-refused",
        join_dialog_page("join-refused", FIXTURE_TICKET, Some(INVALID_TICKET_TEXT)),
    );
    render_page_at(
        "join-dialog-minimum-size",
        join_dialog_page("join-minimum", FIXTURE_TICKET, None),
        MINIMUM_WINDOW_SIZE_PX,
    );
}

fn readable_ticket() -> String {
    let mut transport = IrohTransport::new(IrohConfig {
        use_relays_and_lookup: false,
        local_lookup_port: SNAPSHOT_LOOKUP_PORT,
    })
    .expect("endpoint binds");
    transport.host().expect("invite").as_str().to_owned()
}

fn console_page(name: &str, task: ConsoleTask) -> Page {
    let mut page = home_page(name);
    if task == ConsoleTask::Joining {
        page.forms.join_ticket = readable_ticket();
    }
    page.forms.console = Some(ConsoleRun {
        task,
        clock: ConsoleClock::new(0.0),
        started_at: Instant::now(),
    });
    page
}

fn host_answered(page: &mut Page) {
    page.forms.host_result = Some(Ok(FIXTURE_TICKET.to_owned()));
    page.launcher.link = hosting_alone_link();
    set_net_state(
        page,
        NetState::Hosting {
            ticket: FIXTURE_TICKET.to_owned(),
        },
    );
}

fn console_harness(page: Page, size: (u32, u32)) -> Harness<'static, Page> {
    console_harness_scaled(page, size, LauncherScale::Large)
}

fn console_harness_scaled(
    page: Page,
    size: (u32, u32),
    scale: LauncherScale,
) -> Harness<'static, Page> {
    // The console starts on the harness's own clock, after the harness has drawn its first frame.
    let mut page = page;
    let console = page.forms.console.take();
    let mut harness = stepped_harness_scaled(page, MOTION_FRAME_MS, size, scale);
    let now_ms = harness.ctx.input(|input| input.time) * 1000.0;
    harness.state_mut().forms.console = console.map(|run| ConsoleRun {
        task: run.task,
        clock: ConsoleClock::new(now_ms),
        started_at: run.started_at,
    });
    harness
}

fn render_after_frames(name: &str, page: Page, frames: usize, size: (u32, u32)) {
    render_after_frames_scaled(name, page, frames, size, LauncherScale::Large);
}

fn render_after_frames_scaled(
    name: &str,
    page: Page,
    frames: usize,
    size: (u32, u32),
    scale: LauncherScale,
) {
    let mut harness = console_harness_scaled(page, size, scale);
    for _ in 0..frames {
        harness.step();
    }
    save_snapshot(name, &harness.render().expect("off-screen render"));
}

#[test]
fn the_medium_scale_draws_the_same_pages_smaller() {
    let scale = LauncherScale::Medium;
    let root = scratch_dir("home-medium");
    let launcher = with_generated_art(home_launcher(&root));
    let mut harness = stepped_harness_scaled(
        game_page(launcher, NetState::Idle),
        MOTION_FRAME_MS,
        WINDOW_SIZE_PX,
        scale,
    );
    harness.run();
    let image = harness.render().expect("off-screen render");
    assert_eq!((image.width(), image.height()), medium_size_px());
    save_snapshot("home-medium", &image);

    let mut link = lobby_link(1, false, 2);
    ready_partner(&mut link, false);
    let state = net_state_for(&link, false);
    let mut harness = stepped_harness_scaled(
        lobby_page("lobby-two-medium", link, state),
        MOTION_FRAME_MS,
        WINDOW_SIZE_PX,
        scale,
    );
    harness.run();
    let image = harness.render().expect("off-screen render");
    assert_eq!((image.width(), image.height()), medium_size_px());
    save_snapshot("lobby-hosted-two-medium", &image);

    let name = "console-hosting-done-medium";
    let mut page = hosting_a_saved_game(name);
    host_answered(&mut page);
    render_after_frames_scaled(name, page, CONSOLE_DONE_FRAMES, WINDOW_SIZE_PX, scale);
}

fn medium_size_px() -> (u32, u32) {
    let factor = LauncherScale::Medium.factor();
    (
        (WINDOW_SIZE_PX.0 as f32 * factor).round() as u32,
        (WINDOW_SIZE_PX.1 as f32 * factor).round() as u32,
    )
}

fn hosting_a_saved_game(name: &str) -> Page {
    let mut page = console_page(name, ConsoleTask::Hosting);
    page.launcher.slots = fixture_saves();
    page.launcher.selected_slot = Some(FIXTURE_PICKED_SLOT);
    page.launcher.lobby_mode = LOBBY_MODE_CONTINUE;
    page
}

#[test]
fn the_console_while_hosting() {
    for (suffix, size) in [("", WINDOW_SIZE_PX), ("-900x600", MINIMUM_WINDOW_SIZE_PX)] {
        let name = format!("console-hosting-start{suffix}");
        let page = console_page(&name, ConsoleTask::Hosting);
        render_after_frames(&name, page, CONSOLE_START_FRAMES, size);
        let name = format!("console-hosting-midway{suffix}");
        let mut page = hosting_a_saved_game(&name);
        let mut told = host_link_told(8);
        told.retain(|(ms, _)| *ms < 600);
        set_net_progress(&mut page, &told);
        render_after_frames(&name, page, CONSOLE_DONE_FRAMES, size);
        let name = format!("console-hosting-done{suffix}");
        let mut page = hosting_a_saved_game(&name);
        set_net_progress(&mut page, &host_link_told(8));
        let mut harness = console_harness(page, size);
        for _ in 0..CONSOLE_TICKET_FRAME {
            harness.step();
        }
        host_answered(harness.state_mut());
        for _ in CONSOLE_TICKET_FRAME..CONSOLE_DONE_FRAMES {
            harness.step();
        }
        save_snapshot(&name, &harness.render().expect("off-screen render"));
    }
}

#[test]
fn the_console_while_joining() {
    for (suffix, size) in [("", WINDOW_SIZE_PX), ("-900x600", MINIMUM_WINDOW_SIZE_PX)] {
        let name = format!("console-joining-midway{suffix}");
        let mut page = console_page(&name, ConsoleTask::Joining);
        page.forms.join_result = Some(Ok(()));
        set_net_state(&mut page, NetState::Joining);
        set_net_progress(&mut page, &join_told(u64::MAX));
        render_after_frames(&name, page, CONSOLE_JOIN_MIDWAY_FRAMES, size);
        let name = format!("console-joining-failed{suffix}");
        let mut page = console_page(&name, ConsoleTask::Joining);
        page.forms.join_result = Some(Ok(()));
        set_net_state(&mut page, NetState::Joining);
        set_net_progress(&mut page, &join_told(702));
        let mut harness = console_harness(page, size);
        for _ in 0..CONSOLE_REFUSAL_FRAME {
            harness.step();
        }
        let refusal = RejectReason::VersionMismatch;
        let page = harness.state_mut();
        page.launcher.link.state = ConnectionState::Rejected(refusal);
        set_net_state(
            page,
            NetState::Failed(judge_rejection_text(refusal).to_owned()),
        );
        for _ in CONSOLE_REFUSAL_FRAME..CONSOLE_JOIN_MIDWAY_FRAMES {
            harness.step();
        }
        save_snapshot(&name, &harness.render().expect("off-screen render"));
    }
}

#[test]
fn the_console_with_motion_off() {
    let mut page = hosting_a_saved_game("console-motion-off");
    page.launcher.config.motion = false;
    let mut harness = stepped_harness(page, MOTION_FRAME_MS);
    harness.step();
    save_snapshot(
        "console-motion-off",
        &harness.render().expect("off-screen render"),
    );
}

#[test]
fn hosting_ends_in_the_lobby() {
    let mut page = console_page("console-to-lobby", ConsoleTask::Hosting);
    host_answered(&mut page);
    let mut harness = stepped_harness(page, MOTION_FRAME_MS);
    harness.run();
    let page = harness.state();
    assert!(page.forms.console.is_none());
    assert_eq!(page.launcher.screen, Screen::Game);
    assert_eq!(page.tab, Tab::NewGame);
}

#[test]
fn motion_of_the_boot_console() {
    let page = hosting_a_saved_game("motion-console");
    let mut harness = console_harness(page, WINDOW_SIZE_PX);
    let folder = motion_folder("boot-console");
    record_frames(
        &mut harness,
        &folder,
        0,
        CONSOLE_CLIP_FRAMES,
        |harness, index| {
            if index == CONSOLE_CLIP_WORK_DONE_FRAME {
                host_answered(harness.state_mut());
            }
        },
    );
    assert_settles(&mut harness);
}

#[test]
fn a_guest_joined_from_the_other_tab_follows_the_host() {
    for (tab, host_kind, name) in [
        (Tab::NewGame, LOBBY_MODE_CONTINUE, "from-new-game"),
        (Tab::Continue, LOBBY_MODE_NEW_GAME, "from-continue"),
    ] {
        let snapshot = format!("lobby-guest-{name}");
        let mut link = lobby_link(1, true, 2);
        ready_partner(&mut link, true);
        for member in &mut link.members {
            if let Some(lobby) = member.lobby.as_mut() {
                lobby.mode = host_kind;
            }
        }
        let state = net_state_for(&link, false);
        let mut page = lobby_page(&snapshot, link, state);
        page.tab = tab;
        page.launcher.lobby_mode = if tab == Tab::Continue {
            LOBBY_MODE_CONTINUE
        } else {
            LOBBY_MODE_NEW_GAME
        };
        page.forms.join_ticket = FIXTURE_TICKET.to_owned();
        page.launcher.follow_the_hosts_kind();
        assert_eq!(page.launcher.lobby_mode, host_kind);
        render_page(&snapshot, page);
    }
}

#[test]
fn lobby_hosting_alone() {
    let state = NetState::Hosting {
        ticket: FIXTURE_TICKET.to_owned(),
    };
    let page = lobby_page("lobby-alone", hosting_alone_link(), state);
    render_page("lobby-hosting-alone", page);
}

#[test]
fn hosted_lobby_of_two() {
    let mut link = lobby_link(1, false, 2);
    ready_partner(&mut link, false);
    let state = net_state_for(&link, false);
    render_page("lobby-hosted-two", lobby_page("lobby-two", link, state));
}

#[test]
fn lobby_with_the_ticket_copied() {
    let mut link = lobby_link(1, false, 2);
    ready_partner(&mut link, true);
    let state = net_state_for(&link, false);
    let mut page = lobby_page("lobby-copied", link, state);
    page.ticket_copied_once = true;
    render_page("lobby-ticket-copied", page);
}

#[test]
fn lobby_seen_by_the_guest() {
    let mut link = lobby_link(1, true, 2);
    ready_partner(&mut link, true);
    let state = net_state_for(&link, false);
    let mut page = lobby_page("lobby-guest", link, state);
    page.forms.join_ticket = FIXTURE_TICKET.to_owned();
    render_page("lobby-guest-view", page);
}

#[test]
fn the_guest_always_plays_the_swapped_body() {
    let mut link = lobby_link(1, true, 2);
    ready_partner(&mut link, true);
    let state = net_state_for(&link, false);
    let mut page = lobby_page("lobby-guest-body-swapped", link, state);
    page.forms.join_ticket = FIXTURE_TICKET.to_owned();
    render_page("lobby-guest-body-swapped", page);
}

#[test]
fn a_party_too_big_for_the_campaign() {
    let mut link = lobby_link(1, false, 2);
    link.members.push(MemberStatus::new(2, "Okabe".to_owned()));
    ready_partner(&mut link, true);
    let state = net_state_for(&link, false);
    render_page(
        "lobby-party-does-not-fit",
        lobby_page("lobby-misfit", link, state),
    );
}

#[test]
fn lobby_at_the_minimum_window_size() {
    let link = lobby_link(1, false, 2);
    let state = net_state_for(&link, false);
    render_page_at(
        "lobby-minimum-size",
        lobby_page("lobby-minimum", link, state),
        MINIMUM_WINDOW_SIZE_PX,
    );
}

#[test]
fn shared_parts_sheet() {
    let (width, height) = WINDOW_SIZE_PX;
    let mut harness = Harness::builder()
        .with_size(vec2(width as f32, height as f32))
        .with_accessibility_check(false)
        .renderer(WgpuTestRenderer::new())
        .with_context_setup(apply_launcher_look)
        .build_ui(|ui| {
            let window = ui.ctx().content_rect();
            ui.scope_builder(UiBuilder::new().max_rect(window), draw_parts_sheet);
        });
    harness.run_steps(PARTS_SHEET_FRAMES);
    save_snapshot("parts-sheet", &harness.render().expect("off-screen render"));
}

fn fixture_slot(index: u8, name: &str, level: i32, hours: f64) -> SlotSummary {
    SlotSummary {
        index,
        used: !name.is_empty(),
        name: name.to_owned(),
        phase: "FIXTURE_PHASE".to_owned(),
        level,
        play_seconds: hours * 3600.0,
    }
}

#[test]
fn settings() {
    let root = scratch_dir("settings");
    let mut cards = every_game_found(&root);
    for card in &mut cards {
        card.ready_build = Some(FIXTURE_READY_BUILD.to_owned());
    }
    let mut launcher = with_generated_art(fixture_launcher(
        &REPLICANT,
        cards,
        LinkStatus::default(),
        Screen::Game,
    ));
    launcher.normal_slots = vec![
        fixture_slot(0, "", 0, 0.0),
        fixture_slot(1, "Mira", 12, 7.5),
        fixture_slot(2, "Okabe", 31, 22.17),
    ];
    launcher.copy_from_slot = Some(2);
    launcher.config.update_url = Some(FIXTURE_BUILD_SERVER.to_owned());
    launcher.backups = vec![
        "20261112-134000".to_owned(),
        "20261111-220500".to_owned(),
        "20261109-191200".to_owned(),
    ];
    launcher.selected_backup = launcher.backups.first().cloned();
    let mut page = game_page(launcher, NetState::Idle);
    page.tab = Tab::Settings;
    render_page("settings-from-home", page);
}

#[test]
fn logs() {
    let root = scratch_dir("logs");
    let mut launcher = fixture_launcher(
        &REPLICANT,
        every_game_found(&root),
        LinkStatus::default(),
        Screen::Logs,
    );
    launcher.log = FIXTURE_LOG_LINES
        .iter()
        .map(|line| (*line).to_owned())
        .collect();
    launcher.export_note = Some("Exported to ~/fixture/logs".to_owned());
    let page = game_page(with_generated_art(launcher), NetState::Idle);
    render_page("logs", page);
}

fn continue_page(name: &str, as_guest: bool) -> Page {
    let mut link = lobby_link(1, as_guest, 2);
    ready_partner(&mut link, true);
    for member in &mut link.members {
        if let Some(lobby) = member.lobby.as_mut() {
            lobby.mode = LOBBY_MODE_CONTINUE;
            lobby.slot = Some(1);
        }
    }
    let state = net_state_for(&link, false);
    let mut page = lobby_page(name, link, state);
    page.launcher.lobby_mode = LOBBY_MODE_CONTINUE;
    page.tab = Tab::Continue;
    page
}

#[test]
fn continue_as_the_host_with_a_save_picked() {
    let mut page = continue_page("continue-host", false);
    page.launcher.slots = fixture_saves();
    page.launcher.selected_slot = Some(2);
    render_page("continue-host", page);
}

#[test]
fn continue_as_the_guest_with_the_hosts_campaign() {
    let mut page = continue_page("continue-guest", true);
    page.forms.join_ticket = FIXTURE_TICKET.to_owned();
    fixture_campaign_card(
        &mut page.launcher,
        CampaignCard {
            campaign_id: FIXTURE_CAMPAIGN_ID.to_owned(),
            host: SaveSummary {
                name: "Okabe".to_owned(),
                level: 31,
                play_seconds: 22.17 * 3600.0,
            },
            second_seat: Some(SecondSeatCard {
                summary: SaveSummary {
                    name: "Mira".to_owned(),
                    level: 28,
                    play_seconds: 19.5 * 3600.0,
                },
                body: Some(PartnerBody::SeedKaineE),
            }),
        },
    );
    render_page("continue-guest", page);
}

#[test]
fn continue_as_the_guest_before_the_card_arrives() {
    let mut page = continue_page("continue-guest-waiting", true);
    page.forms.join_ticket = FIXTURE_TICKET.to_owned();
    render_page("continue-guest-waiting", page);
}

fn motion_harness(page: Page, frame_ms: f32) -> Harness<'static, Page> {
    let mut harness = stepped_harness(page, frame_ms);
    harness.run();
    harness
}

fn stepped_harness(page: Page, frame_ms: f32) -> Harness<'static, Page> {
    stepped_harness_at(page, frame_ms, WINDOW_SIZE_PX)
}

fn stepped_harness_at(page: Page, frame_ms: f32, size: (u32, u32)) -> Harness<'static, Page> {
    stepped_harness_scaled(page, frame_ms, size, LauncherScale::Large)
}

fn stepped_harness_scaled(
    mut page: Page,
    frame_ms: f32,
    (width, height): (u32, u32),
    scale: LauncherScale,
) -> Harness<'static, Page> {
    page.launcher.new_game_name = FIXTURE_PLAYER_NAME.to_owned();
    Harness::builder()
        .with_size(vec2(width as f32, height as f32))
        .with_accessibility_check(false)
        .renderer(WgpuTestRenderer::new())
        .with_context_setup(move |ctx| {
            apply_launcher_look(ctx);
            ctx.set_zoom_factor(scale.factor());
        })
        .with_step_dt(frame_ms / 1000.0)
        .with_max_steps(MOTION_SETTLE_MAX_STEPS)
        .build_ui_state(draw_page, page)
}

fn motion_folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(SNAPSHOT_FOLDER)
        .join(MOTION_FOLDER)
        .join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("motion folder");
    folder
}

fn record_frames(
    harness: &mut Harness<'static, Page>,
    folder: &Path,
    first: usize,
    frames: usize,
    mut before_frame: impl FnMut(&mut Harness<'static, Page>, usize),
) -> usize {
    for index in 0..frames {
        before_frame(harness, index);
        harness.step();
        let image = harness.render().expect("off-screen render");
        image
            .save(folder.join(format!("{:03}.png", first + index)))
            .expect("png written");
    }
    first + frames
}

fn assert_settles(harness: &mut Harness<'static, Page>) {
    // The first run lets egui finish its own layout passes; after it an idle page asks for no frame.
    harness.run();
    let steps = harness.run();
    assert_eq!(
        steps, MOTION_SETTLED_STEPS,
        "the page kept asking for frames once its motion ended"
    );
}

fn record_motion(
    name: &str,
    page: Page,
    frames: usize,
    change: impl FnOnce(&mut Page),
) -> Harness<'static, Page> {
    let mut harness = motion_harness(page, MOTION_FRAME_MS);
    let folder = motion_folder(name);
    change(harness.state_mut());
    record_frames(&mut harness, &folder, 0, frames, |_, _| {});
    assert_settles(&mut harness);
    harness
}

fn set_net_state(page: &mut Page, state: NetState) {
    let progress = page.forms.net.progress();
    page.forms.net = Arc::new(FixtureNet { state, progress });
}

fn set_net_progress(page: &mut Page, told: &[(u64, LinkProgress)]) {
    let started_at = page
        .forms
        .console
        .as_ref()
        .map_or_else(Instant::now, |run| run.started_at);
    let progress = told
        .iter()
        .map(|(ms, progress)| ProgressEntry {
            at: started_at + Duration::from_millis(*ms),
            progress: progress.clone(),
        })
        .collect();
    let state = page.forms.net.status().state;
    page.forms.net = Arc::new(FixtureNet { state, progress });
}

fn address_kind(ipv6: bool, reach: AddressReach) -> AddressKind {
    AddressKind { ipv6, reach }
}

fn own_link_told(from_ms: u64) -> Vec<(u64, LinkProgress)> {
    vec![
        (
            from_ms,
            LinkProgress::Bound {
                ipv4_port: Some(51820),
                ipv6_port: Some(51821),
            },
        ),
        (from_ms, LinkProgress::OwnId("4b1d0c9e2f".to_owned())),
        (
            from_ms + 230,
            LinkProgress::RelayChosen {
                relay: FIXTURE_RELAY.to_owned(),
            },
        ),
        (
            from_ms + 250,
            LinkProgress::DirectAddressFound {
                kind: address_kind(false, AddressReach::LocalNetwork),
                count: 1,
            },
        ),
        (
            from_ms + 260,
            LinkProgress::DirectAddressFound {
                kind: address_kind(true, AddressReach::LocalNetwork),
                count: 2,
            },
        ),
        (
            from_ms + 610,
            LinkProgress::RelayAnswered {
                relay: FIXTURE_RELAY.to_owned(),
                after: Duration::from_millis(380),
            },
        ),
        (
            from_ms + 640,
            LinkProgress::DirectAddressFound {
                kind: address_kind(false, AddressReach::Public),
                count: 3,
            },
        ),
    ]
}

fn host_link_told(from_ms: u64) -> Vec<(u64, LinkProgress)> {
    let mut told = own_link_told(from_ms);
    told.extend([
        (
            from_ms + 1,
            LinkProgress::LocalLookupAnswering {
                port: LOCAL_LOOKUP_PORT,
            },
        ),
        (
            from_ms + 450,
            LinkProgress::KeyPublished {
                after: Duration::from_millis(630),
            },
        ),
    ]);
    told.sort_by_key(|(ms, _)| *ms);
    told
}

fn join_told(until_ms: u64) -> Vec<(u64, LinkProgress)> {
    let mut told = vec![(
        4,
        LinkProgress::TicketRead {
            host_id: "9f3c2a71e0".to_owned(),
            relay: None,
            direct_addresses: 0,
        },
    )];
    told.extend(own_link_told(6));
    told.extend([
        (8, LinkProgress::LookingUpHost),
        (
            420,
            LinkProgress::HostFound {
                by: LookupMeans::PublishedRecord,
                after: Duration::from_millis(412),
                relay: Some(FIXTURE_RELAY.to_owned()),
                direct_addresses: 0,
            },
        ),
        (
            700,
            LinkProgress::HostAnswered {
                after: Duration::from_millis(280),
            },
        ),
        (701, LinkProgress::PathTried(PathRoute::Relayed)),
        (
            701,
            LinkProgress::PathWon {
                route: PathRoute::Relayed,
                rtt: Some(Duration::from_millis(FIXTURE_RELAY_ROUND_TRIP_MS)),
            },
        ),
        (702, LinkProgress::HelloSent { protocol: 67 }),
        (
            890,
            LinkProgress::PathTried(PathRoute::Direct(address_kind(false, AddressReach::Public))),
        ),
    ]);
    told.sort_by_key(|(ms, _)| *ms);
    told.retain(|(ms, _)| *ms <= until_ms);
    told
}

fn hosting_alone_page(name: &str) -> Page {
    let state = NetState::Hosting {
        ticket: FIXTURE_TICKET.to_owned(),
    };
    lobby_page(name, hosting_alone_link(), state)
}

#[test]
fn motion_of_the_selection_sweep() {
    let page = hosting_alone_page("motion-sweep");
    record_motion("selection-sweep", page, 18, |page| {
        page.tab = Tab::Continue;
    });
}

#[test]
fn motion_of_a_tab_change() {
    let page = hosting_alone_page("motion-tab");
    record_motion("tab-change", page, 30, |page| page.tab = Tab::Settings);
}

#[test]
fn motion_of_the_copy_line() {
    let mut link = lobby_link(1, false, 2);
    ready_partner(&mut link, true);
    let state = net_state_for(&link, false);
    let page = lobby_page("motion-copy", link, state);
    let mut harness = motion_harness(page, MOTION_FRAME_MS);
    let folder = motion_folder("copy-line");
    harness.state_mut().ticket_copied_once = true;
    let hold_frames = (COPY_HOLD_FRAMES_SECONDS * 1000.0 / MOTION_FRAME_MS) as usize;
    record_frames(&mut harness, &folder, 0, hold_frames, |_, _| {});
    assert_settles(&mut harness);
}

#[test]
fn motion_of_entering_and_leaving_a_lobby() {
    let page = home_page("motion-lobby");
    let mut harness = record_motion("lobby-enter", page, 45, |page| {
        host_answered(page);
        page.launcher.screen = Screen::Game;
        page.tab = Tab::NewGame;
    });
    let folder = motion_folder("lobby-leave");
    let page = harness.state_mut();
    set_net_state(page, NetState::Idle);
    let last_seen = std::mem::take(&mut page.launcher.link);
    page.launcher
        .session_ends
        .push((SessionEndCause::PlayerLeft, last_seen));
    record_frames(&mut harness, &folder, 0, 25, |_, _| {});
    assert_settles(&mut harness);
    assert_eq!(harness.state().launcher.screen, Screen::Home);
}

#[test]
fn motion_of_a_seat_filling_and_a_status_word() {
    let state = NetState::Hosting {
        ticket: FIXTURE_TICKET.to_owned(),
    };
    let page = lobby_page("motion-seat", hosting_alone_link(), state);
    let mut harness = record_motion("seat-fill", page, 40, |page| {
        let mut link = lobby_link(1, false, 2);
        ready_partner(&mut link, false);
        set_net_state(page, net_state_for(&link, false));
        page.launcher.link = link;
    });
    let folder = motion_folder("status-word");
    ready_partner(&mut harness.state_mut().launcher.link, true);
    record_frames(&mut harness, &folder, 0, 16, |_, _| {});
    assert_settles(&mut harness);
}

#[test]
fn motion_of_the_open_seat_while_hosting() {
    let state = NetState::Hosting {
        ticket: FIXTURE_TICKET.to_owned(),
    };
    let page = lobby_page("motion-drift", hosting_alone_link(), state);
    let mut harness = motion_harness(page, DRIFT_FRAME_MS);
    let folder = motion_folder("hatch-drift");
    record_frames(&mut harness, &folder, 0, DRIFT_FRAMES, |_, _| {});
    assert_settles(&mut harness);
}

#[test]
fn motion_of_the_home_rows() {
    let root = scratch_dir("motion-home");
    let page = game_page(home_launcher(&root), NetState::Idle);
    let mut harness = motion_harness(page, MOTION_FRAME_MS);
    let folder = motion_folder("home-art-load");
    harness.state_mut().launcher = with_generated_art(home_launcher(&root));
    record_frames(&mut harness, &folder, 0, 20, |_, _| {});
    assert_settles(&mut harness);
    let folder = motion_folder("home-hover");
    let next = record_frames(&mut harness, &folder, 0, 16, |harness, index| {
        if index == 2 {
            harness.hover_at(HOME_ROW_ART_POINT);
        }
    });
    record_frames(&mut harness, &folder, next, 16, |harness, index| {
        if index == 2 {
            harness.hover_at(HOME_AWAY_POINT);
        }
    });
    assert_settles(&mut harness);
}

#[test]
fn no_motion_draws_the_end_state_in_the_first_frame() {
    let mut page = hosting_alone_page("motion-off");
    page.launcher.config.motion = false;
    let mut harness = motion_harness(page, MOTION_FRAME_MS);
    harness.state_mut().tab = Tab::Settings;
    harness.step();
    let first = harness.render().expect("off-screen render");
    assert_settles(&mut harness);
    let settled = harness.render().expect("off-screen render");
    assert!(
        first.as_raw() == settled.as_raw(),
        "a frame moved with motion off"
    );
}

fn home_with_saves_in_scratch(name: &str, with_coop_saves: bool) -> Page {
    let root = scratch_dir(name);
    let mut launcher = home_launcher(&root);
    let prefix = root.join("prefix");
    for card in &mut launcher.cards {
        if let Some(install) = card.install.as_mut() {
            install.proton_prefix = Some(prefix.clone());
        }
    }
    let install = launcher.cards[0].install.clone().expect("found game");
    let folder = save_dir(&install)
        .expect("save folder")
        .join(FIXTURE_STEAM_ACCOUNT_FOLDER);
    std::fs::create_dir_all(&folder).expect("scratch save folder");
    std::fs::write(folder.join("GAMEDATA"), b"own save").expect("own save");
    if with_coop_saves {
        std::fs::write(folder.join("GAMEDATA.coop"), b"co-op save").expect("co-op save");
        std::fs::create_dir_all(folder.join("GAMEDATA.coop.seats")).expect("seats");
    }
    game_page(with_generated_art(launcher), NetState::Idle)
}

fn right_click_at(harness: &mut Harness<'static, Page>, point: egui::Pos2) {
    harness.hover_at(point);
    harness.run_steps(2);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(2);
    }
}

fn scale_suffix(scale: LauncherScale) -> &'static str {
    match scale {
        LauncherScale::Large => "",
        LauncherScale::Medium => "-medium",
    }
}

#[test]
fn right_clicking_the_row_offers_to_delete_the_co_op_saves_and_asks_again() {
    for scale in LauncherScale::ALL {
        let suffix = scale_suffix(scale);
        let page = home_with_saves_in_scratch(&format!("coop-save-menu{suffix}"), true);
        let mut harness = stepped_harness_scaled(page, MOTION_FRAME_MS, WINDOW_SIZE_PX, scale);
        harness.run();
        right_click_at(&mut harness, HOME_ROW_RIGHT_CLICK_POINT);
        let menu = harness
            .state()
            .launcher
            .coop_save_menu
            .clone()
            .expect("menu open");
        assert_eq!(menu.coop_saves, 2);
        assert!(!menu.confirming);
        save_snapshot(
            &format!("home-coop-save-menu{suffix}"),
            &harness.render().expect("off-screen render"),
        );
        click_at(&mut harness, HOME_ROW_RIGHT_CLICK_POINT + MENU_ENTRY_OFFSET);
        let menu = harness
            .state()
            .launcher
            .coop_save_menu
            .clone()
            .expect("still open");
        assert!(menu.confirming);
        save_snapshot(
            &format!("home-coop-save-confirm{suffix}"),
            &harness.render().expect("off-screen render"),
        );
        harness.key_press(egui::Key::Escape);
        harness.run_steps(2);
        assert!(harness.state().launcher.coop_save_menu.is_none());
    }
}

#[test]
fn with_no_co_op_save_the_delete_entry_is_dim_and_does_nothing() {
    for scale in LauncherScale::ALL {
        let suffix = scale_suffix(scale);
        let page = home_with_saves_in_scratch(&format!("coop-save-dim{suffix}"), false);
        let mut harness = stepped_harness_scaled(page, MOTION_FRAME_MS, WINDOW_SIZE_PX, scale);
        harness.run();
        right_click_at(&mut harness, HOME_ROW_RIGHT_CLICK_POINT);
        assert_eq!(
            harness
                .state()
                .launcher
                .coop_save_menu
                .as_ref()
                .map(|menu| menu.coop_saves),
            Some(0)
        );
        click_at(&mut harness, HOME_ROW_RIGHT_CLICK_POINT + MENU_ENTRY_OFFSET);
        let menu = harness
            .state()
            .launcher
            .coop_save_menu
            .clone()
            .expect("still open");
        assert!(!menu.confirming);
        save_snapshot(
            &format!("home-coop-save-dim{suffix}"),
            &harness.render().expect("off-screen render"),
        );
    }
}

#[test]
fn a_click_outside_the_co_op_save_menu_closes_it() {
    let page = home_with_saves_in_scratch("coop-save-click-away", true);
    let mut harness = stepped_harness(page, MOTION_FRAME_MS);
    harness.run();
    right_click_at(&mut harness, HOME_ROW_RIGHT_CLICK_POINT);
    assert!(harness.state().launcher.coop_save_menu.is_some());
    click_at(&mut harness, HOME_AWAY_POINT);
    assert!(harness.state().launcher.coop_save_menu.is_none());
}
