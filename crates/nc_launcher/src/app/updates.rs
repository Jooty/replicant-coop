use std::path::{Path, PathBuf};

use eframe::egui::{Context, ViewportCommand};

use crate::DEV_BUILD;
use crate::app::{Launcher, Task, run_install};
use crate::install::{ModStatus, game_running};
use crate::steam::GameInstall;
use crate::update::{
    AvailableUpdate, Channel, UpdateError, apply_hook_update, apply_update, build_config,
    check_for_update, own_build, short_commit,
};

pub const RESTART_TAB_ARGUMENT: &str = "--tab";
pub const RESTART_TAB_SETTINGS: &str = "settings";
pub const RESTART_TAB_LOGS: &str = "logs";
pub const RESTART_UPDATED_ARGUMENT: &str = "--updated";

type CheckResult = Result<Option<AvailableUpdate>, UpdateError>;
type ApplyResult = Result<Vec<String>, UpdateError>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Updated(String),
    Available(Box<AvailableUpdate>),
    Updating,
    Failed(String),
}

#[derive(Default)]
pub struct Updater {
    pub state: UpdateState,
    check_task: Option<Task<CheckResult>>,
    apply_task: Option<Task<ApplyResult>>,
    started_at_launch: bool,
    mod_update_requested: bool,
    updating_to: Option<String>,
    pub server_text: Option<String>,
    exe_before_update: Option<PathBuf>,
}

impl Launcher {
    pub fn update_url(&self) -> Option<String> {
        let build = build_config();
        let configured = (build.channel == Channel::Dev)
            .then_some(self.config.update_url.as_deref())
            .flatten();
        configured
            .or(build.update_url.as_deref())
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_owned)
    }

    /// The short build the build server offers for the game known by `game_id`, when it offers one.
    pub fn server_build_for(&self, game_id: &str) -> Option<String> {
        match &self.updater.state {
            UpdateState::Available(update) if update.plan.hook_for(game_id).is_some() => {
                Some(short_commit(&update.manifest.commit).to_owned())
            }
            _ => None,
        }
    }

    pub fn start_update_check(&mut self, ctx: &Context) {
        let Some(url) = self.update_url() else {
            return;
        };
        if matches!(
            self.updater.state,
            UpdateState::Checking | UpdateState::Updating
        ) {
            return;
        }
        self.updater.state = UpdateState::Checking;
        let public_key_hex = build_config().public_key_hex.clone();
        self.updater.check_task = Some(Task::spawn(
            move || check_for_update(&url, &public_key_hex, &own_build(), std::env::consts::OS),
            ctx.clone(),
        ));
    }

    pub fn start_mod_update(&mut self, ctx: &Context) {
        if self.update_url().is_none() {
            if !self.any_game_running {
                self.start_install(ctx);
            }
            return;
        }
        if self.any_game_running {
            self.updater.state = UpdateState::Failed("close the game first".to_owned());
            return;
        }
        self.updater.mod_update_requested = true;
        self.start_update_check(ctx);
    }

    pub fn start_update_apply(&mut self, ctx: &Context, install_mod: bool) {
        let UpdateState::Available(update) = self.updater.state.clone() else {
            return;
        };
        if install_mod && self.any_game_running {
            self.updater.state = UpdateState::Failed("close the game first".to_owned());
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            self.updater.state = UpdateState::Failed("could not find the launcher file".to_owned());
            return;
        };
        let mod_installed = matches!(self.mod_status, ModStatus::Current | ModStatus::Outdated);
        let reinstall_into = (!self.any_game_running && (install_mod || mod_installed))
            .then(|| self.game.clone())
            .flatten();
        self.log.push(format!(
            "updating to {}...",
            short_commit(&update.manifest.commit)
        ));
        self.updater.state = UpdateState::Updating;
        self.updater.updating_to = Some(short_commit(&update.manifest.commit).to_owned());
        self.updater.exe_before_update = Some(exe.clone());
        self.updater.apply_task = Some(Task::spawn(
            move || run_update(&update, &exe, reinstall_into, DEV_BUILD),
            ctx.clone(),
        ));
    }

    pub(super) fn tick_updates(&mut self, ctx: &Context) {
        if !self.updater.started_at_launch {
            self.updater.started_at_launch = true;
            if !DEV_BUILD && self.config.check_for_updates && self.update_url().is_some() {
                self.start_update_check(ctx);
            }
        }
        if let Some(result) = self.updater.check_task.as_ref().and_then(Task::try_take) {
            self.updater.check_task = None;
            self.finish_check(result, ctx);
        }
        if let Some(result) = self.updater.apply_task.as_ref().and_then(Task::try_take) {
            self.updater.apply_task = None;
            self.finish_apply(result, ctx);
        }
    }

    fn finish_check(&mut self, result: CheckResult, ctx: &Context) {
        let mod_update_requested = std::mem::take(&mut self.updater.mod_update_requested);
        match result {
            Ok(Some(update)) => {
                self.log.push(format!(
                    "update available: {}",
                    short_commit(&update.manifest.commit)
                ));
                self.updater.state = UpdateState::Available(Box::new(update));
                if mod_update_requested {
                    self.start_update_apply(ctx, true);
                }
            }
            Ok(None) => {
                self.updater.state = UpdateState::UpToDate;
                if mod_update_requested {
                    self.log
                        .push("the build server has nothing newer".to_owned());
                    self.start_install(ctx);
                }
            }
            Err(error) => {
                self.log.push(format!("update check failed: {error}"));
                self.updater.state = UpdateState::Failed(error.to_string());
                if mod_update_requested {
                    self.log
                        .push("installing the mod that came with the launcher".to_owned());
                    self.start_install(ctx);
                }
            }
        }
    }

    fn finish_apply(&mut self, result: ApplyResult, ctx: &Context) {
        match result {
            Ok(lines) => {
                self.log.extend(lines);
                let short = self.updater.updating_to.take().unwrap_or_default();
                self.log.push(format!("Mod updated to {short}"));
                self.updater.state = UpdateState::Updated(short.clone());
                if DEV_BUILD {
                    self.refresh();
                } else {
                    restart_launcher(self.updater.exe_before_update.take(), &short, ctx);
                }
            }
            Err(error) => {
                self.log.push(format!("update failed: {error}"));
                self.updater.state = UpdateState::Failed(error.to_string());
            }
        }
    }
}

fn run_update(
    update: &AvailableUpdate,
    exe: &Path,
    reinstall_into: Option<GameInstall>,
    hook_only: bool,
) -> ApplyResult {
    let mut lines = if hook_only {
        apply_hook_update(update, exe)?;
        vec!["downloaded and verified the new hook dll".to_owned()]
    } else {
        apply_update(update, exe)?;
        vec!["downloaded and verified the new launcher and hook dll".to_owned()]
    };
    if reinstall_into
        .as_ref()
        .is_some_and(|game| game_running(game.profile))
    {
        lines.push("the game started during the update, the mod was not reinstalled".to_owned());
        return Ok(lines);
    }
    if let Some(game) = reinstall_into
        && let Some(dll) = exe.parent().map(|dir| dir.join(game.profile.hook_dll_name))
    {
        let report = run_install(game, Some(dll));
        lines.extend(report.lines);
    }
    Ok(lines)
}

fn restart_launcher(exe: Option<PathBuf>, updated_to: &str, ctx: &Context) {
    if let Some(exe) = exe {
        let mut kept = Vec::new();
        let mut skip_value = false;
        for argument in std::env::args_os().skip(1) {
            let text = argument.to_string_lossy();
            if skip_value {
                skip_value = false;
            } else if text == RESTART_TAB_ARGUMENT || text == RESTART_UPDATED_ARGUMENT {
                skip_value = true;
            } else {
                kept.push(argument.clone());
            }
        }
        let _ = std::process::Command::new(exe)
            .args(kept)
            .args([RESTART_TAB_ARGUMENT, RESTART_TAB_SETTINGS])
            .args([RESTART_UPDATED_ARGUMENT, updated_to])
            .spawn();
    }
    ctx.send_viewport_cmd(ViewportCommand::Close);
}
