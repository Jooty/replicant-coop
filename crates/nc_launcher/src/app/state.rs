use std::path::PathBuf;

use eframe::egui::Context;

use nc_launcher_core::{
    ALL_PROFILES, GameProfile, REPLICANT, SeatNameChoice, SeatNameInputs, SessionEndCause,
    followed_lobby_kind, hosting_kind, own_seat_name,
};

use replicant_proto::{GUEST_BODY, GameReadiness, HOST_BODY, PartnerBody};

use crate::app::actions::ledger_path;
use crate::app::event_log_line;
use crate::app::{
    ActionReport, GAME_POLL_SECONDS, LogUploader, STEAM_RUN_URL_PREFIX, Task, Updater, run_install,
    run_uninstall,
};
use crate::app::{CoopSaveMenu, Screen};
use crate::config::data_dir;
use crate::config::{ConfigRole, LauncherConfig, config_path, process_config_home};
use crate::game_link::{
    clear_boot, clear_link, write_boot, write_coop_save_file, write_dev_switches, write_link,
    write_log, write_new_game, write_settings,
};
use crate::home::{BannerArtStore, GameCard, PortraitArtStore, build_card};
use crate::install::{
    GAME_NEVER_APPEARED_SECONDS, GameWatch, GameWatchStep, ModStatus, end_processes, game_pids,
    game_running, mod_mismatch, open_url,
};
use crate::lobby::{
    FollowNotice, GUEST_WAIT_FOR_HOST_GAME_SECONDS, GuestWaitStep, GuestWaiting, HostClosedInputs,
    LastLaunch, LaunchMode, NameError, PlayerStatus, QuitFallback, QuitStep, ReadyInputs,
    guest_accepts_start, host_game_closed, lobby_ready, new_game_slot, own_close_notice,
    player_status, relaunch_choice, resolve_mode, validate_name,
};
use crate::logs::{
    LAUNCHER_LOG_BEFORE_ROLE, LogWriter, game_log_name, launcher_log_name, logs_dir,
    path_as_the_game_sees_it, rotate_log,
};
use crate::net::{
    ConnectionState, DEFAULT_PLAYER_NAME, LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, LauncherEvent,
    LauncherNet, LinkStatus, LobbyReady, MemberChange, PlayerSettings, SessionOptions,
};
use crate::saves::{
    SlotError, SlotSummary, backup_coop_file, backup_root, backup_saves, coop_backup_root,
    coop_copy_target_slot, coop_gamedata_path, copy_slot_into_coop, default_slot,
    drop_lost_campaigns, format_stamp, fresh_coop_file, gamedata_path, list_backups,
    read_coop_slots, read_slots, restore_coop_backup, save_dir, seat_backups_dir, slot_label,
    slot_player_name,
};
use crate::seat_round::{GuestSeatRound, HostSeatRound, HostStartStep};
use crate::second_copy::{
    EARLY_EXIT_SECONDS, LOGS_FOLDER_NAME, SecondCopy, build_command, start_second_copy,
};
use crate::steam::GameInstall;
use nc_launcher_core::StartGateStep;
use replicant_proto::CampaignCard;

const MAX_LOG_LINES: usize = 5000;
const HOST_MEMBER_SLOT: u8 = 0;
const GAME_NOT_FOUND_TEXT: &str = "Game not found. Set the folder in Settings.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Guest,
}

struct ResolvedLaunch {
    name: String,
    boot_slot: Option<u8>,
    new_game: Option<(u8, String)>,
    coop_save_file: Option<String>,
}

pub struct Launcher {
    pub config: LauncherConfig,
    pub profile: &'static GameProfile,
    pub screen: Screen,
    pub cards: Vec<GameCard>,
    pub banner_art: BannerArtStore,
    pub portrait_art: PortraitArtStore,
    pub game: Option<GameInstall>,
    pub dll_source: Option<PathBuf>,
    pub mod_status: ModStatus,
    pub any_game_running: bool,
    pub own_game_running: bool,
    pub follow_notice: Option<FollowNotice>,
    pub coop_save_menu: Option<CoopSaveMenu>,
    /// Session ends the network told of, with the link as it last was, for the page to act on.
    pub session_ends: Vec<(SessionEndCause, LinkStatus)>,
    pub log: Vec<String>,
    pub export_note: Option<String>,
    second_copy: Option<SecondCopy>,
    pub override_text: String,
    pub new_game_name: String,
    pub lobby_mode: u8,
    /// The kind of game a guest's lobby switched to by following the host, until the shown tab follows it.
    pub tab_to_follow: Option<u8>,
    pub lobby_started: bool,
    pub link_port: Option<u16>,
    pub work: Option<Task<ActionReport>>,
    pub updater: Updater,
    pub log_uploader: LogUploader,
    pub link: LinkStatus,
    pub slots: Vec<SlotSummary>,
    pub normal_slots: Vec<SlotSummary>,
    pub copy_from_slot: Option<u8>,
    pub selected_slot: Option<u8>,
    pub host_options_seen: Option<SessionOptions>,
    guest_waiting: Option<GuestWaiting>,
    game_watch: Option<GameWatch>,
    last_launch: Option<LastLaunch>,
    quit_fallback: Option<QuitFallback>,
    quit_request_pending: bool,
    own_readiness: GameReadiness,
    game_closed: bool,
    game_linked_since_launch: bool,
    partner_game_seen_linked: bool,
    pub backups: Vec<String>,
    pub selected_backup: Option<String>,
    log_writer: Option<LogWriter>,
    log_written: usize,
    role_log_chosen: bool,
    last_game_poll: f64,
    host_round: Option<HostSeatRound>,
    guest_round: Option<GuestSeatRound>,
    pub(super) pending_host_start: Option<u8>,
    seat_flush_due: bool,
}

impl Launcher {
    pub fn load() -> Self {
        let config = config_path()
            .map(|path| LauncherConfig::load(&path))
            .unwrap_or_default();
        let log_writer =
            logs_dir().and_then(|dir| LogWriter::start(&dir, LAUNCHER_LOG_BEFORE_ROLE).ok());
        let mut launcher = Self::from_parts(config, log_writer);
        launcher.log_config_home();
        launcher.refresh();
        launcher
    }

    pub(crate) fn from_parts(config: LauncherConfig, log_writer: Option<LogWriter>) -> Self {
        let launcher_started_at = std::time::SystemTime::now();
        let override_text = config
            .game_dir_for(&REPLICANT)
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();
        let new_game_name = config
            .player_name
            .clone()
            .unwrap_or_else(|| DEFAULT_PLAYER_NAME.to_owned());
        Self {
            config,
            profile: &REPLICANT,
            screen: Screen::Home,
            cards: Vec::new(),
            banner_art: BannerArtStore::default(),
            portrait_art: PortraitArtStore::default(),
            game: None,
            dll_source: None,
            mod_status: ModStatus::NotInstalled,
            any_game_running: false,
            own_game_running: false,
            follow_notice: None,
            coop_save_menu: None,
            session_ends: Vec::new(),
            last_launch: None,
            quit_fallback: None,
            quit_request_pending: false,
            log: Vec::new(),
            export_note: None,
            second_copy: None,
            override_text,
            new_game_name,
            lobby_mode: LOBBY_MODE_NEW_GAME,
            tab_to_follow: None,
            lobby_started: false,
            link_port: None,
            work: None,
            updater: Updater::default(),
            log_uploader: LogUploader::new(launcher_started_at),
            link: LinkStatus::default(),
            slots: Vec::new(),
            normal_slots: Vec::new(),
            copy_from_slot: None,
            selected_slot: None,
            host_options_seen: None,
            guest_waiting: None,
            game_watch: None,
            own_readiness: GameReadiness::NotLinked,
            game_closed: false,
            game_linked_since_launch: false,
            partner_game_seen_linked: false,
            backups: Vec::new(),
            selected_backup: None,
            log_writer,
            log_written: 0,
            role_log_chosen: false,
            last_game_poll: f64::NEG_INFINITY,
            host_round: None,
            guest_round: None,
            pending_host_start: None,
            seat_flush_due: false,
        }
    }

    fn log_config_home(&mut self) {
        let Some(home) = process_config_home() else {
            return;
        };
        match home.role {
            ConfigRole::Second => self.log.push(format!(
                "another launcher is already running on this PC: this one keeps its settings in {}{}",
                home.dir.display(),
                if home.seeded_from_main {
                    " (copied from the first launcher's file)"
                } else {
                    ""
                }
            )),
            ConfigRole::Override => self.log.push(format!(
                "settings folder set by the environment: {}",
                home.dir.display()
            )),
            ConfigRole::First => {}
        }
    }

    pub fn refresh(&mut self) {
        let ledger = ledger_path();
        self.cards = ALL_PROFILES
            .iter()
            .map(|profile| build_card(profile, &self.config, ledger.as_deref()))
            .collect();
        let active = self
            .cards
            .iter()
            .find(|card| card.profile.game_id == self.profile.game_id)
            .cloned();
        self.game = active.as_ref().and_then(|card| card.install.clone());
        self.dll_source = active.as_ref().and_then(|card| card.dll_source.clone());
        self.mod_status = active.map_or(ModStatus::NotInstalled, |card| card.mod_status);
        self.any_game_running = game_running(self.profile);
        self.refresh_saves();
    }

    pub fn can_switch_game(&self) -> bool {
        !self.lobby_started && self.game_watch.is_none() && !self.busy()
    }

    pub fn open_game(&mut self, profile: &'static GameProfile) {
        if !self.select_game(profile) {
            return;
        }
        self.screen = Screen::Game;
        self.refresh();
    }

    /// Makes `profile` the launcher's game without leaving the page, when a switch is allowed now.
    pub fn choose_game(&mut self, profile: &'static GameProfile) -> bool {
        if !self.select_game(profile) {
            return false;
        }
        self.refresh();
        true
    }

    /// Picks the kind of game HOST opens the lobby on, for the game chosen now.
    pub fn take_hosting_kind(&mut self) -> u8 {
        let has_coop_save = self.profile.support.saves && self.slots.iter().any(|slot| slot.used);
        self.lobby_mode = hosting_kind(has_coop_save);
        self.lobby_mode
    }

    /// The two checks hosting makes before it asks for a session: the game's folder, and the mod's files.
    pub fn hosting_checks(&self) -> (Result<(), String>, Result<(), String>) {
        let folder = if self.game.is_some() {
            Ok(())
        } else {
            Err(GAME_NOT_FOUND_TEXT.to_owned())
        };
        let mod_files = if self.mod_status == ModStatus::Current || self.dll_source.is_some() {
            Ok(())
        } else {
            Err(self.missing_hook_text())
        };
        (folder, mod_files)
    }

    fn missing_hook_text(&self) -> String {
        format!(
            "No {} next to the launcher to install.",
            self.profile.hook_dll_name
        )
    }

    pub fn install_mod_from_home(&mut self, profile: &'static GameProfile, ctx: &Context) {
        if self.busy() || !self.select_game(profile) {
            return;
        }
        self.refresh();
        self.start_install(ctx);
    }

    fn select_game(&mut self, profile: &'static GameProfile) -> bool {
        if profile.game_id == self.profile.game_id {
            return true;
        }
        if !self.can_switch_game() {
            return false;
        }
        self.profile = profile;
        self.override_text = self
            .config
            .game_dir_for(profile)
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();
        true
    }

    pub fn go_home(&mut self) {
        self.screen = Screen::Home;
        self.refresh();
    }

    pub fn refresh_saves(&mut self) {
        self.backups = coop_backup_root()
            .map(|root| list_backups(&root))
            .unwrap_or_default();
        self.normal_slots = self
            .game
            .as_ref()
            .and_then(save_dir)
            .and_then(|dir| read_slots(&dir).ok())
            .unwrap_or_default();
        if !self.copy_from_slot.is_some_and(|picked| {
            self.normal_slots
                .iter()
                .any(|slot| slot.index == picked && slot.used)
        }) {
            self.copy_from_slot = default_slot(&self.normal_slots);
        }
        self.slots = match self
            .game
            .as_ref()
            .and_then(save_dir)
            .map(|dir| read_coop_slots(&dir))
        {
            Some(Ok(slots)) => slots,
            Some(Err(SlotError::Io(error))) if error.kind() == std::io::ErrorKind::NotFound => {
                Vec::new()
            }
            Some(Err(error)) => {
                self.log
                    .push(format!("could not read the save slots: {error}"));
                Vec::new()
            }
            None => Vec::new(),
        };
        self.set_aside_lost_campaigns();
        self.ensure_seat_rounds();
        let still_valid = self.selected_slot.is_some_and(|picked| {
            self.slots
                .iter()
                .any(|slot| slot.index == picked && slot.used)
        });
        if !still_valid {
            self.selected_slot = default_slot(&self.slots);
        }
    }

    pub fn tick(&mut self, now_seconds: f64, ctx: &Context) {
        if let Some(report) = self.work.as_ref().and_then(Task::try_take) {
            self.work = None;
            if let Some(path) = report.fetched_dll.clone() {
                self.dll_source = Some(path);
            }
            self.log.extend(report.lines);
            self.refresh();
        }
        self.banner_art.load_new_cards(&self.cards, ctx);
        self.tick_updates(ctx);
        self.tick_log_upload(ctx);
        self.poll_second_copy();
        self.write_new_log_lines();
        if self.log.len() > MAX_LOG_LINES {
            let dropped = self.log.len() - MAX_LOG_LINES;
            self.log.drain(..dropped);
            self.log_written = self.log_written.saturating_sub(dropped);
        }
        if now_seconds - self.last_game_poll >= GAME_POLL_SECONDS {
            self.last_game_poll = now_seconds;
            self.poll_game_process(now_seconds, ctx);
        }
        self.step_guest_wait(now_seconds);
        self.step_follow_host_game(now_seconds);
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(GAME_POLL_SECONDS));
    }

    pub fn busy(&self) -> bool {
        self.work.is_some()
    }

    pub fn start_install(&mut self, ctx: &Context) {
        let Some(game) = self.game.clone() else {
            return;
        };
        let source = self.dll_source.clone();
        self.log.push("installing...".to_owned());
        self.work = Some(Task::spawn(move || run_install(game, source), ctx.clone()));
    }

    pub fn start_uninstall(&mut self, ctx: &Context) {
        let Some(game) = self.game.clone() else {
            return;
        };
        self.log.push("uninstalling...".to_owned());
        self.work = Some(Task::spawn(move || run_uninstall(game), ctx.clone()));
    }

    pub fn player_name(&self) -> String {
        self.config
            .player_name
            .clone()
            .unwrap_or_else(|| DEFAULT_PLAYER_NAME.to_owned())
    }

    pub fn effective_player_name(&self) -> String {
        self.own_seat_name().name
    }

    /// The co-op save picked to continue, when one is.
    pub fn selected_save(&self) -> Option<&SlotSummary> {
        let picked = self.selected_slot?;
        self.slots
            .iter()
            .find(|slot| slot.index == picked && slot.used)
    }

    /// The name on the own seat, the one the session and the game get, and whether it may be typed into.
    pub fn own_seat_name(&self) -> SeatNameChoice {
        let typed = validate_name(&self.new_game_name).ok();
        let picked = slot_player_name(&self.slots, self.selected_slot);
        let stored = self.player_name();
        let card_seat_name = self
            .campaign_card()
            .and_then(|card| card.second_seat.as_ref())
            .map(|seat| seat.summary.name.as_str());
        own_seat_name(SeatNameInputs {
            mode: self.lobby_mode,
            viewer_is_guest: self.link.guest,
            partner_seated: !self.link.members.is_empty(),
            picked_save_name: picked.as_deref(),
            hosts_card_seat_name: card_seat_name,
            typed_name: typed.as_deref(),
            stored_name: &stored,
        })
    }

    pub fn name_check(&self) -> Result<String, NameError> {
        validate_name(&self.new_game_name)
    }

    pub fn lobby_ready_as(&self, is_guest: bool) -> LobbyReady {
        lobby_ready(ReadyInputs {
            mode: self.lobby_mode,
            game_ok: self.can_start_game(),
            name_ok: self.name_check().is_ok(),
            new_game_slot_free: new_game_slot(&self.slots).is_some(),
            continue_slot: if is_guest && self.guest_plays_a_seat() {
                Some(0)
            } else {
                self.selected_slot
            },
            is_guest,
        })
    }

    pub fn own_lobby_body(&self) -> PartnerBody {
        if self.link.guest {
            GUEST_BODY
        } else {
            HOST_BODY
        }
    }

    pub fn own_lobby_ready(&self) -> LobbyReady {
        self.lobby_ready_as(self.link.guest)
    }

    pub fn note_events(&mut self, events: &[LauncherEvent], net: &dyn LauncherNet) {
        let game_name = self.profile.display_name;
        // A session's end is logged by the page, once it knows where the launcher went.
        self.log.extend(
            events
                .iter()
                .filter(|event| !matches!(event, LauncherEvent::SessionEnded { .. }))
                .map(|event| event_log_line(event, game_name)),
        );
        for event in events {
            self.feed_seat_rounds(event, net);
            match event {
                LauncherEvent::GameChangedSettings { settings } => {
                    self.config.take_player_settings(*settings);
                    self.apply_settings();
                }
                LauncherEvent::SessionOptionsChanged { options } if self.link.guest => {
                    self.host_options_seen = Some(*options);
                }
                LauncherEvent::SessionOptionsChanged { options } => {
                    self.config.take_session_options(*options);
                    self.apply_settings();
                }
                LauncherEvent::OwnGameReadiness { readiness } => self.own_readiness = *readiness,
                LauncherEvent::MemberChanged {
                    change: MemberChange::Readiness(readiness),
                    ..
                } => {
                    self.partner_game_seen_linked |= *readiness != GameReadiness::NotLinked;
                }
                LauncherEvent::GameLinked => self.game_linked_since_launch = true,
                LauncherEvent::GameUnlinked { .. }
                    if self.game_linked_since_launch && self.lobby_started =>
                {
                    self.mark_game_closed();
                }
                LauncherEvent::LobbyStartReceived { .. } => self.start_as_guest(),
                LauncherEvent::SessionEnded { cause, last_seen } => {
                    self.session_ends.push((*cause, last_seen.clone()));
                    self.lobby_started = false;
                    self.guest_waiting = None;
                    self.partner_game_seen_linked = false;
                    self.follow_notice = None;
                    self.quit_fallback = None;
                }
                LauncherEvent::MemberLeft { .. }
                | LauncherEvent::Stopped
                | LauncherEvent::Failed { .. } => {
                    self.lobby_started = false;
                    self.guest_waiting = None;
                    self.partner_game_seen_linked = false;
                    self.follow_notice = None;
                    self.quit_fallback = None;
                }
                _ => {}
            }
        }
    }

    pub fn release_game_link(&mut self) {
        if let Some(copy) = self.second_copy.take() {
            copy.kill_tree();
        }
        if let Some(game) = &self.game
            && let Err(error) = clear_link(game.profile, &game.game_dir)
                .and_then(|()| clear_boot(game.profile, &game.game_dir))
        {
            self.log
                .push(format!("could not clean up the link: {error}"));
        }
    }

    pub fn can_start_game(&self) -> bool {
        self.game.is_some()
            && self.profile.support.start
            && (!self.any_game_running || crate::DEV_BUILD)
            && !self.busy()
            && (self.mod_status == ModStatus::Current || self.dll_source.is_some())
    }

    fn start_as_guest(&mut self) {
        let own = self.own_lobby_ready();
        let connected = self.link.state == ConnectionState::Connected;
        if !guest_accepts_start(connected, self.link.guest, self.lobby_started, own) {
            self.log.push(
                "ignored a start from the host: not connected, not ready or already started"
                    .to_owned(),
            );
            return;
        }
        self.begin_game_launch();
        self.lobby_started = true;
        self.guest_waiting = Some(GuestWaiting::new(own.mode));
        self.log
            .push("waiting for the host's game to start".to_owned());
    }

    fn begin_game_launch(&mut self) {
        self.game_closed = false;
        self.follow_notice = None;
        self.game_linked_since_launch = false;
        self.partner_game_seen_linked = self.link.partner_readiness() != GameReadiness::NotLinked;
    }

    fn step_guest_wait(&mut self, now_seconds: f64) {
        let Some(waiting) = self.guest_waiting.as_mut() else {
            return;
        };
        let mode = waiting.mode;
        match waiting.step(self.link.partner_readiness(), now_seconds) {
            GuestWaitStep::KeepWaiting => {}
            GuestWaitStep::LaunchNow => {
                self.guest_waiting = None;
                self.lobby_started = false;
                self.launch(Role::Guest, mode);
            }
            GuestWaitStep::GiveUp => {
                self.guest_waiting = None;
                self.lobby_started = false;
                self.fail_launch(format!(
                    "the host's game did not start within {GUEST_WAIT_FOR_HOST_GAME_SECONDS:.0} s"
                ));
            }
        }
    }

    fn poll_game_process(&mut self, now_seconds: f64, ctx: &Context) {
        let pids = game_pids(self.profile);
        self.any_game_running = !pids.is_empty();
        self.own_game_running = self
            .game_watch
            .as_ref()
            .is_some_and(|watch| !watch.pids_running(&pids).is_empty());
        let Some(watch) = self.game_watch.as_mut() else {
            return;
        };
        match watch.step(&pids, now_seconds) {
            GameWatchStep::Waiting | GameWatchStep::Running => {}
            GameWatchStep::Exited => {
                self.game_watch = None;
                self.log.push("the game closed".to_owned());
                self.mark_game_closed();
                self.upload_after_game(ctx);
            }
            GameWatchStep::NeverAppeared => {
                self.game_watch = None;
                self.fail_launch(format!(
                    "the game did not start within {GAME_NEVER_APPEARED_SECONDS:.0} s"
                ));
                self.mark_game_closed();
            }
        }
    }

    fn mark_game_closed(&mut self) {
        let session_up = self.link.state == ConnectionState::Connected;
        let host_close_in_progress = self.quit_fallback.take().is_some()
            || self.follow_notice == Some(FollowNotice::HostGameClosed);
        let already_closed = self.game_closed;
        self.game_watch = None;
        self.own_game_running = false;
        self.seat_flush_due |= self.link.guest;
        self.game_closed = true;
        self.lobby_started = false;
        self.own_readiness = GameReadiness::NotLinked;
        if already_closed {
            return;
        }
        self.follow_notice = own_close_notice(session_up, host_close_in_progress);
        match self.follow_notice {
            Some(FollowNotice::HostGameClosed) => {
                self.log
                    .push("follow: this game is closed, as the host's game closed".to_owned());
            }
            Some(FollowNotice::YourGameClosed) if self.link.guest => self.log.push(
                "follow: your game closed during a session, relaunch and rejoin is offered"
                    .to_owned(),
            ),
            Some(FollowNotice::YourGameClosed) => self.log.push(
                "follow: your game closed during a session, the guest's game will be closed"
                    .to_owned(),
            ),
            None => {}
        }
    }

    pub fn take_quit_request(&mut self) -> bool {
        std::mem::take(&mut self.quit_request_pending)
    }

    fn step_follow_host_game(&mut self, now_seconds: f64) {
        let host_closed = host_game_closed(HostClosedInputs {
            guest: self.link.guest,
            session_up: self.link.state == ConnectionState::Connected,
            lobby_started: self.lobby_started,
            waiting_for_host_game: self.guest_waiting.is_some(),
            own_game_closed: self.game_closed,
            partner_seen_linked: self.partner_game_seen_linked,
            partner: self.link.partner_readiness(),
            notice_shown: self.follow_notice.is_some(),
        });
        if host_closed {
            self.log
                .push("follow: the host's game closed, asking this game to quit".to_owned());
            self.follow_notice = Some(FollowNotice::HostGameClosed);
            self.quit_request_pending = true;
            self.quit_fallback = Some(QuitFallback::new(now_seconds));
        }
        let Some(fallback) = self.quit_fallback.as_mut() else {
            return;
        };
        if fallback.step(now_seconds) != QuitStep::EndProcesses {
            return;
        }
        let pids = self
            .game_watch
            .as_ref()
            .map(|watch| watch.pids_running(&game_pids(self.profile)))
            .unwrap_or_default();
        if let Some(copy) = &self.second_copy {
            self.log
                .push("follow: the game did not quit in time, ending the second copy".to_owned());
            copy.force_kill_tree();
        }
        if pids.is_empty() {
            if self.second_copy.is_none() {
                self.log.push(
                    "follow: the game did not quit in time and no watched process is left to end"
                        .to_owned(),
                );
            }
            return;
        }
        self.log.push(format!(
            "follow: the game did not quit in time, ending process {pids:?}"
        ));
        end_processes(&pids);
    }

    pub fn can_relaunch_and_rejoin(&self) -> bool {
        self.link.guest
            && self.follow_notice == Some(FollowNotice::YourGameClosed)
            && self.last_launch.is_some()
            && self.link.state == ConnectionState::Connected
    }

    pub fn relaunch_and_rejoin(&mut self) {
        let Some(last) = self.last_launch.filter(|_| self.can_relaunch_and_rejoin()) else {
            return;
        };
        let (mode, slot) = relaunch_choice(last, |slot| {
            self.slots
                .iter()
                .any(|summary| summary.index == slot && summary.used)
        });
        if let Some(slot) = slot {
            self.selected_slot = Some(slot);
        }
        self.lobby_mode = mode;
        self.log.push(format!(
            "follow: relaunching your game (mode {mode}, slot {}), waiting for the host's game",
            slot.map_or_else(|| "none".to_owned(), slot_label)
        ));
        self.begin_game_launch();
        self.lobby_started = true;
        self.guest_waiting = Some(GuestWaiting::new(mode));
    }

    fn remember_launch(&mut self, role: Role, mode: u8, resolved: &ResolvedLaunch) {
        let slot = resolved
            .boot_slot
            .or(resolved.new_game.as_ref().map(|(slot, _)| *slot));
        self.last_launch = Some(LastLaunch {
            guest: role == Role::Guest,
            mode,
            slot,
        });
    }

    pub fn your_player_status(&self) -> PlayerStatus {
        player_status(
            self.own_lobby_ready().ready,
            self.own_readiness,
            self.game_closed,
        )
    }

    pub fn their_player_status(&self) -> PlayerStatus {
        let partner = self.link.partner();
        let ready = partner
            .and_then(|partner| partner.lobby)
            .is_some_and(|lobby| lobby.ready);
        let readiness = self.link.partner_readiness();
        let closed = self.partner_game_seen_linked && readiness == GameReadiness::NotLinked;
        player_status(ready, readiness, closed)
    }

    pub fn launch(&mut self, role: Role, mode: u8) -> bool {
        self.switch_launcher_log(role);
        self.note_run_started(role);
        self.refresh();
        if role == Role::Host {
            self.begin_game_launch();
        }
        if role == Role::Guest
            && mode == LOBBY_MODE_CONTINUE
            && self.guest_plays_a_seat()
            && self.seat_launch().is_none()
        {
            self.fail_launch(
                "The host's save for your seat was not written on this machine (see the log), so the game was not started."
                    .to_owned(),
            );
            return false;
        }
        let Some(resolved) = self.resolve_launch(mode) else {
            self.fail_launch(
                "could not start: no valid name or save slot for this mode".to_owned(),
            );
            return false;
        };
        self.lobby_started = true;
        if self.any_game_running {
            self.launch_second_copy(role, &resolved);
            if self.lobby_started {
                self.remember_launch(role, mode, &resolved);
            }
            return self.lobby_started;
        }
        if self.mod_status != ModStatus::Current
            && let Some(game) = self.game.clone()
        {
            let report = run_install(game, self.dll_source.clone());
            let failed = report.failed;
            self.log.extend(report.lines);
            self.refresh();
            if failed {
                self.lobby_started = false;
                return false;
            }
        }
        let log_path = self.start_game_log(role);
        if let (Some(game), Some(port)) = (&self.game, self.link_port) {
            if let Err(error) = write_link(game.profile, &game.game_dir, port, &resolved.name) {
                self.log
                    .push(format!("could not write the link for the game: {error}"));
            }
            if let Some(log_path) = &log_path
                && let Err(error) = write_log(game.profile, &game.game_dir, log_path)
            {
                self.log
                    .push(format!("could not write the game log path: {error}"));
            }
            self.write_boot_settings(&resolved);
            self.write_settings_file();
        }
        let pids_before_launch = game_pids(self.profile);
        match open_url(&format!(
            "{STEAM_RUN_URL_PREFIX}{}",
            self.profile.steam_app_id
        )) {
            Ok(()) => {
                self.log.push("asked Steam to start the game".to_owned());
                self.remember_launch(role, mode, &resolved);
                self.game_watch = Some(GameWatch::new(&pids_before_launch));
            }
            Err(error) => {
                self.log.push(format!("could not open Steam: {error}"));
                self.lobby_started = false;
                return false;
            }
        }
        true
    }

    fn write_boot_settings(&mut self, resolved: &ResolvedLaunch) {
        let Some(game) = &self.game else {
            return;
        };
        let block_saves = false;
        let coop_file = save_dir(game).and_then(|saves| coop_gamedata_path(&saves));
        self.log.push(match coop_file {
            Some(path) => format!(
                "co-op save file: {} ({})",
                path.display(),
                std::fs::metadata(&path).map_or_else(
                    |error| format!("missing: {error}"),
                    |meta| format!("{} bytes", meta.len())
                )
            ),
            None => "co-op save file: no save folder found".to_owned(),
        });
        let written = match &resolved.new_game {
            Some((slot, name)) => {
                write_new_game(game.profile, &game.game_dir, *slot, name, block_saves)
            }
            None => write_boot(
                game.profile,
                &game.game_dir,
                resolved.boot_slot,
                block_saves,
            ),
        }
        .and_then(|()| {
            write_coop_save_file(
                game.profile,
                &game.game_dir,
                resolved.coop_save_file.as_deref(),
            )
        });
        if let Err(error) = written {
            self.log
                .push(format!("could not write the boot settings: {error}"));
        }
    }

    fn plan_launch(&self, mode: u8) -> Option<ResolvedLaunch> {
        if mode == LOBBY_MODE_CONTINUE && self.link.guest && self.guest_plays_a_seat() {
            return self.seat_launch();
        }
        let new_game_name = self.name_check().ok();
        let plan = resolve_mode(mode, self.selected_slot, new_game_name)?;
        match plan {
            LaunchMode::Continue { slot } => Some(ResolvedLaunch {
                name: slot_player_name(&self.slots, Some(slot))
                    .unwrap_or_else(|| self.effective_player_name()),
                boot_slot: Some(slot),
                new_game: None,
                coop_save_file: None,
            }),
            LaunchMode::NewGame { name } => Some(ResolvedLaunch {
                boot_slot: None,
                new_game: Some((new_game_slot(&self.slots)?, name.clone())),
                name,
                coop_save_file: None,
            }),
        }
    }

    fn resolve_launch(&mut self, mode: u8) -> Option<ResolvedLaunch> {
        let new_game = self.plan_launch(mode)?.new_game.is_some();
        if new_game {
            if !self.any_game_running {
                self.ensure_coop_file();
            }
            self.refresh_saves();
        }
        let resolved = self.plan_launch(mode)?;
        if let Some((slot, name)) = &resolved.new_game {
            self.log.push(format!(
                "starting a new co-op game in slot {} as {name}",
                slot_label(*slot)
            ));
            self.config.player_name = Some(name.clone());
            self.save_config();
        }
        Some(resolved)
    }

    fn ensure_coop_file(&mut self) {
        let Some(saves) = self.game.as_ref().and_then(save_dir) else {
            return;
        };
        let Some(target) = coop_gamedata_path(&saves) else {
            return;
        };
        if target.is_file() {
            return;
        }
        let vanilla = gamedata_path(&saves).and_then(|path| std::fs::read(path).ok());
        match fresh_coop_file(vanilla.as_deref()) {
            Ok(bytes) => match std::fs::write(&target, bytes) {
                Ok(()) => self.log.push("made a fresh co-op save file".to_owned()),
                Err(error) => self
                    .log
                    .push(format!("could not write the co-op save file: {error}")),
            },
            Err(SlotError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                self.log
                    .push("no normal save file yet: the game makes the co-op save".to_owned());
            }
            Err(error) => self
                .log
                .push(format!("could not make a fresh co-op save file: {error}")),
        }
    }

    fn fail_launch(&mut self, message: String) {
        self.log.push(message);
    }

    pub fn player_settings(&self) -> PlayerSettings {
        self.config.player_settings(crate::DEV_BUILD)
    }

    pub fn apply_settings(&mut self) {
        self.save_config();
        self.write_settings_file();
    }

    fn write_settings_file(&mut self) {
        let Some(game) = &self.game else {
            return;
        };
        if let Err(error) = write_settings(
            game.profile,
            &game.game_dir,
            &self.player_settings(),
            &self.config.session_options(),
        ) {
            self.log
                .push(format!("could not write the game settings: {error}"));
        }
        if let Err(error) = write_dev_switches(game.profile, &game.game_dir, crate::DEV_BUILD) {
            self.log
                .push(format!("could not write the dev switches: {error}"));
        }
    }

    fn launch_second_copy(&mut self, role: Role, resolved: &ResolvedLaunch) {
        if !crate::DEV_BUILD {
            self.log.push("the game is already running".to_owned());
            self.lobby_started = false;
            return;
        }
        let (Some(game), Some(port)) = (self.game.clone(), self.link_port) else {
            self.fail_second_copy("no game or no link port for a second copy".to_owned());
            return;
        };
        if let Some(message) =
            mod_mismatch(game.profile, &game.game_dir, self.dll_source.as_deref())
        {
            self.fail_second_copy(message);
            return;
        }
        self.write_boot_settings(resolved);
        self.write_settings_file();
        let log_path = self.start_game_log(role);
        let command = match build_command(
            &game,
            port,
            &resolved.name,
            resolved.boot_slot,
            resolved
                .new_game
                .as_ref()
                .map(|(slot, name)| (*slot, name.as_str())),
            log_path.as_deref(),
        ) {
            Ok(command) => command,
            Err(error) => {
                self.fail_second_copy(format!("could not start a second copy: {error}"));
                return;
            }
        };
        match start_second_copy(&command) {
            Ok(copy) => {
                self.log.push(format!(
                    "started a second game copy as the {}",
                    if role == Role::Host { "host" } else { "guest" }
                ));
                self.second_copy = Some(copy);
            }
            Err(error) => self.fail_second_copy(format!("could not start a second copy: {error}")),
        }
    }

    pub fn copy_launch_command(&mut self, ctx: &Context) {
        let (Some(game), Some(port)) = (self.game.clone(), self.link_port) else {
            self.log
                .push("no game or no link port to build the command".to_owned());
            return;
        };
        let Some(resolved) = self.plan_launch(self.lobby_mode) else {
            self.log
                .push("no valid name or save slot to build the command".to_owned());
            return;
        };
        match build_command(
            &game,
            port,
            &resolved.name,
            resolved.boot_slot,
            resolved
                .new_game
                .as_ref()
                .map(|(slot, name)| (*slot, name.as_str())),
            None,
        ) {
            Ok(command) => {
                ctx.copy_text(command.shell_line());
                self.log.push(format!(
                    "copied the launch command: {}",
                    command.shell_line()
                ));
            }
            Err(error) => self
                .log
                .push(format!("could not build the launch command: {error}")),
        }
    }

    pub fn export_logs(&mut self) {
        let Some(dir) = data_dir().map(|dir| dir.join(LOGS_FOLDER_NAME)) else {
            self.export_note = Some("no data folder on this system".to_owned());
            return;
        };
        let path = dir.join(format!("{}.txt", format_stamp(&chrono::Local::now())));
        let written = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&path, self.log.join("\n") + "\n"));
        self.export_note = Some(match written {
            Ok(()) => format!("saved {}", path.display()),
            Err(error) => format!("could not save the logs: {error}"),
        });
    }

    pub fn open_logs_folder(&mut self) {
        let Some(dir) = logs_dir() else {
            self.export_note = Some("no data folder on this system".to_owned());
            return;
        };
        let opened =
            std::fs::create_dir_all(&dir).and_then(|()| open_url(&dir.display().to_string()));
        if let Err(error) = opened {
            self.export_note = Some(format!("could not open the logs folder: {error}"));
        }
    }

    fn switch_launcher_log(&mut self, role: Role) {
        if self.role_log_chosen {
            return;
        }
        self.role_log_chosen = true;
        self.write_new_log_lines();
        let Some(dir) = logs_dir() else {
            return;
        };
        self.log_writer = LogWriter::start(&dir, launcher_log_name(role)).ok();
        self.log_written = 0;
        self.write_new_log_lines();
    }

    fn start_game_log(&mut self, role: Role) -> Option<String> {
        let path = logs_dir()?.join(game_log_name(role));
        if let Err(error) = std::fs::create_dir_all(path.parent()?).and_then(|()| rotate_log(&path))
        {
            self.log
                .push(format!("could not rotate the game log: {error}"));
        }
        Some(path_as_the_game_sees_it(&path))
    }

    fn write_new_log_lines(&mut self) {
        let Some(writer) = self.log_writer.as_mut() else {
            return;
        };
        for line in self.log.iter().skip(self.log_written) {
            let _ = writer.write_line(line);
        }
        self.log_written = self.log.len();
    }

    fn fail_second_copy(&mut self, message: String) {
        self.lobby_started = false;
        self.log.push(message);
    }

    fn poll_second_copy(&mut self) {
        let Some(copy) = &self.second_copy else {
            return;
        };
        let output = copy.take_output();
        self.log.extend(
            output
                .lines
                .into_iter()
                .map(|line| format!("[copy] {line}")),
        );
        let Some(exit) = output.exit else {
            return;
        };
        self.second_copy = None;
        self.mark_game_closed();
        if exit.seconds < EARLY_EXIT_SECONDS {
            self.log.push(format!(
                "the second copy closed after {:.0} s (exit {}); see second-copy.log",
                exit.seconds,
                exit.code
                    .map_or_else(|| "none".to_owned(), |code| code.to_string())
            ));
        }
    }

    pub fn backup_saves(&mut self) {
        let (Some(game), Some(root)) = (&self.game, backup_root()) else {
            self.log
                .push("no game found, so no saves to back up".to_owned());
            return;
        };
        let Some(saves) = save_dir(game) else {
            self.log
                .push("could not find the Documents folder".to_owned());
            return;
        };
        let stamp = format_stamp(&chrono::Local::now());
        match backup_saves(&saves, &root, &stamp) {
            Ok((_, files)) => self
                .log
                .push(format!("saved backup {stamp} ({files} files)")),
            Err(error) => self.log.push(format!("backup failed: {error}")),
        }
        self.refresh_saves();
    }

    pub fn backup_coop_saves(&mut self) {
        let (Some(game), Some(root)) = (&self.game, coop_backup_root()) else {
            self.log
                .push("no game found, so no co-op saves to back up".to_owned());
            return;
        };
        let Some(saves) = save_dir(game) else {
            self.log
                .push("could not find the Documents folder".to_owned());
            return;
        };
        let stamp = format_stamp(&chrono::Local::now());
        match backup_coop_file(&saves, &root, &stamp) {
            Ok(_) => self.log.push(format!("saved co-op backup {stamp}")),
            Err(error) => self.log.push(format!("co-op backup failed: {error}")),
        }
        self.refresh_saves();
    }

    pub fn restore_selected_backup(&mut self) {
        let (Some(game), Some(root), Some(stamp)) =
            (&self.game, coop_backup_root(), self.selected_backup.clone())
        else {
            return;
        };
        if self.any_game_running {
            self.log
                .push("close the game before restoring saves".to_owned());
            return;
        }
        let Some(saves) = save_dir(game) else {
            self.log
                .push("could not find the Documents folder".to_owned());
            return;
        };
        let now = format_stamp(&chrono::Local::now());
        match restore_coop_backup(&saves, &root, &stamp, &now) {
            Ok(_) => self.log.push(format!(
                "restored co-op backup {stamp}; the co-op saves it replaced are in before-restore-{now}"
            )),
            Err(error) => self.log.push(format!("restore failed: {error}")),
        }
        self.refresh_saves();
    }

    pub fn copy_normal_slot_into_coop(&mut self) {
        let (Some(game), Some(from)) = (&self.game, self.copy_from_slot) else {
            return;
        };
        if self.any_game_running {
            self.log
                .push("close the game before copying a save".to_owned());
            return;
        }
        let Some(saves) = save_dir(game) else {
            self.log
                .push("could not find the Documents folder".to_owned());
            return;
        };
        let to = match coop_copy_target_slot(&saves) {
            Ok(slot) => slot,
            Err(error) => {
                self.log.push(format!("copy refused: {error}"));
                return;
            }
        };
        let stamp = format_stamp(&chrono::Local::now());
        if let Some(root) = coop_backup_root()
            && coop_gamedata_path(&saves).is_some_and(|path| path.is_file())
            && let Err(error) = backup_coop_file(&saves, &root, &format!("before-copy-{stamp}"))
        {
            self.log.push(format!(
                "copy refused: could not back up the co-op file first ({error})"
            ));
            return;
        }
        match copy_slot_into_coop(&saves, from, to) {
            Ok(_) => {
                self.log.push(format!(
                    "copied normal slot {} into co-op slot {}",
                    slot_label(from),
                    slot_label(to)
                ));
            }
            Err(error) => self.log.push(format!("copy failed: {error}")),
        }
        self.refresh_saves();
    }

    pub fn apply_override(&mut self) {
        let text = self.override_text.trim();
        let dir = (!text.is_empty()).then(|| PathBuf::from(text));
        self.config.set_game_dir_for(self.profile, dir);
        self.save_config();
        self.refresh();
    }

    pub fn find_game(&mut self) {
        self.override_text.clear();
        self.apply_override();
        match &self.game {
            Some(game) => {
                self.override_text = game.game_dir.display().to_string();
                self.log.push("found the game".to_owned());
            }
            None => self
                .log
                .push("could not find the game: type the folder".to_owned()),
        }
    }

    /// Once joined, a guest's lobby runs the host's kind of game (new or saved), whichever tab it joined from.
    pub fn follow_the_hosts_kind(&mut self) {
        if !self.link.guest || self.link.state != ConnectionState::Connected {
            return;
        }
        let host_kind = self
            .link
            .member(HOST_MEMBER_SLOT)
            .and_then(|host| host.lobby)
            .map(|lobby| lobby.mode);
        let kind = followed_lobby_kind(self.lobby_mode, true, host_kind);
        if kind == self.lobby_mode {
            return;
        }
        self.log.push(format!(
            "lobby: following the host's kind of game (mode {kind})"
        ));
        self.lobby_mode = kind;
        self.tab_to_follow = Some(kind);
    }

    pub fn guest_plays_a_seat(&self) -> bool {
        self.lobby_mode == LOBBY_MODE_CONTINUE
            && self
                .guest_round
                .as_ref()
                .is_some_and(GuestSeatRound::seat_expected)
    }

    pub fn campaign_card(&self) -> Option<&CampaignCard> {
        self.guest_round.as_ref()?.card()
    }

    #[cfg(feature = "snapshots")]
    pub(crate) fn hold_campaign_card(&mut self, card: CampaignCard) {
        let mut round = GuestSeatRound::new(None, Vec::new(), PathBuf::new());
        round.on_event(
            &crate::net::StubNet,
            &LauncherEvent::CampaignCardReceived { card },
        );
        self.guest_round = Some(round);
    }

    pub fn host_start_waiting(&self) -> bool {
        self.pending_host_start.is_some()
    }

    pub fn press_start(&mut self, net: &dyn LauncherNet, mode: u8, now_seconds: f64) {
        self.refresh_saves();
        let slot = self.selected_slot.and_then(|picked| {
            self.slots
                .iter()
                .find(|slot| slot.index == picked && slot.used)
                .cloned()
        });
        let guest = self.link.partner().map(|partner| partner.slot);
        let step = match (
            mode == LOBBY_MODE_CONTINUE,
            slot,
            guest,
            self.host_round.as_mut(),
        ) {
            (true, Some(slot), Some(guest), Some(round)) => {
                round.begin_start(net, guest, &slot, now_seconds)
            }
            _ => HostStartStep::LaunchNow,
        };
        self.take_round_logs();
        match step {
            HostStartStep::LaunchNow => {
                if self.launch(Role::Host, mode) {
                    net.start_lobby(mode);
                }
            }
            HostStartStep::Waiting => {
                self.pending_host_start = Some(mode);
            }
            HostStartStep::Stop(reason) => self.fail_launch(reason),
        }
    }

    pub fn step_seat_round(&mut self, net: &dyn LauncherNet, now_seconds: f64) {
        if let Some(step) = self
            .host_round
            .as_mut()
            .and_then(|round| round.tick(now_seconds))
        {
            self.on_start_gate(step, net);
        }
        if std::mem::take(&mut self.seat_flush_due)
            && let Some(round) = self.guest_round.as_mut()
        {
            round.flush(net);
        }
        self.send_card_for_pick(net);
        self.take_round_logs();
    }

    pub fn before_leaving(&mut self, net: &dyn LauncherNet) {
        if self.link.guest
            && let Some(round) = self.guest_round.as_mut()
        {
            round.flush(net);
        }
        self.pending_host_start = None;
        self.take_round_logs();
    }

    fn feed_seat_rounds(&mut self, event: &LauncherEvent, net: &dyn LauncherNet) {
        let saved_on_the_host =
            !self.link.guest && matches!(event, LauncherEvent::GameSaved { .. });
        if !saved_on_the_host && let Some(round) = self.guest_round.as_mut() {
            round.on_event(net, event);
        }
        let step = if self.link.guest {
            None
        } else {
            self.host_round
                .as_mut()
                .and_then(|round| round.on_event(net, event))
        };
        self.take_round_logs();
        if let Some(step) = step {
            self.on_start_gate(step, net);
        }
    }

    fn on_start_gate(&mut self, step: StartGateStep, net: &dyn LauncherNet) {
        let Some(mode) = self.pending_host_start.take() else {
            return;
        };
        match step {
            StartGateStep::Launch => {
                if self.launch(Role::Host, mode) {
                    net.start_lobby(mode);
                }
            }
            StartGateStep::Stop(reason) => self.fail_launch(reason),
            StartGateStep::Wait => self.pending_host_start = Some(mode),
        }
    }

    fn send_card_for_pick(&mut self, net: &dyn LauncherNet) {
        let open = matches!(
            self.link.state,
            ConnectionState::Hosting | ConnectionState::Connected
        );
        if self.link.guest
            || !open
            || self.lobby_mode != LOBBY_MODE_CONTINUE
            || self.pending_host_start.is_some()
        {
            return;
        }
        let Some(slot) = self.selected_slot.and_then(|picked| {
            self.slots
                .iter()
                .find(|slot| slot.index == picked && slot.used)
                .cloned()
        }) else {
            return;
        };
        if let Some(round) = self.host_round.as_mut()
            && round.card_slot() != Some(slot.index)
        {
            round.pick(net, &slot);
        }
    }

    fn take_round_logs(&mut self) {
        if let Some(round) = self.host_round.as_mut() {
            self.log.append(&mut round.log);
        }
        if let Some(round) = self.guest_round.as_mut() {
            self.log.append(&mut round.log);
        }
    }

    fn seat_launch(&self) -> Option<ResolvedLaunch> {
        let file = self.guest_round.as_ref()?.session_file_name()?.to_owned();
        let name = self
            .campaign_card()
            .and_then(|card| card.second_seat.as_ref())
            .map(|seat| seat.summary.name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.player_name());
        Some(ResolvedLaunch {
            name,
            boot_slot: Some(0),
            new_game: None,
            coop_save_file: Some(file),
        })
    }

    fn set_aside_lost_campaigns(&mut self) {
        if self.any_game_running || self.slots.is_empty() {
            return;
        }
        let Some(coop) = self
            .game
            .as_ref()
            .and_then(save_dir)
            .and_then(|dir| coop_gamedata_path(&dir))
        else {
            return;
        };
        match drop_lost_campaigns(&coop, &self.slots) {
            Ok(moved) => {
                for aside in moved {
                    self.log.push(format!(
                        "co-op slot {} no longer holds its campaign's save; the campaign and its second seat were moved aside ({})",
                        aside.host_slot,
                        aside
                            .seat
                            .or(aside.record)
                            .map_or_else(|| "nothing stored".to_owned(), |path| path.display().to_string())
                    ));
                }
            }
            Err(error) => self
                .log
                .push(format!("could not move a lost campaign aside: {error}")),
        }
    }

    fn ensure_seat_rounds(&mut self) {
        let saves = self.game.as_ref().and_then(save_dir);
        let coop = saves.as_deref().and_then(coop_gamedata_path);
        match (&coop, &self.host_round) {
            (Some(path), Some(round)) if round.coop_file() == path.as_path() => {}
            (Some(path), _) => self.host_round = Some(HostSeatRound::new(path.clone())),
            (None, _) => self.host_round = None,
        }
        let folder = coop
            .as_ref()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf));
        let current = self.guest_round.as_ref().map(GuestSeatRound::save_folder);
        if current == Some(folder.as_deref()) {
            return;
        }
        let Some(backups) = data_dir().map(|dir| seat_backups_dir(&dir)) else {
            return;
        };
        let templates = [saves.as_deref().and_then(gamedata_path), coop.clone()]
            .into_iter()
            .flatten()
            .collect();
        self.guest_round = Some(GuestSeatRound::new(folder, templates, backups));
    }

    pub(super) fn save_config(&mut self) {
        let Some(path) = config_path() else {
            return;
        };
        if let Err(error) = self.config.save(&path) {
            self.log.push(format!("could not save settings: {error}"));
        }
    }
}
