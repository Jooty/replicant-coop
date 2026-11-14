use std::sync::Arc;
use std::time::Instant;

use eframe::egui::Context;
use nc_launcher_core::{ConsoleClock, ConsoleTask};

use crate::app::{ConsoleRun, EndedLobby, JoinDialog, LobbyFailure, Task};
use crate::net::{LauncherNet, NetError, NetState, SessionOptions};

pub struct NetForms {
    pub net: Arc<dyn LauncherNet>,
    pub join_ticket: String,
    pub join_dialog: Option<JoinDialog>,
    pub console: Option<ConsoleRun>,
    pub lobby_failure: Option<LobbyFailure>,
    pub ended_lobby: Option<EndedLobby>,
    pub session_options: SessionOptions,
    pub host_result: Option<Result<String, NetError>>,
    pub join_result: Option<Result<(), NetError>>,
    host_task: Option<Task<Result<String, NetError>>>,
    join_task: Option<Task<Result<(), NetError>>>,
    abandoned: bool,
    closing: bool,
}

impl NetForms {
    pub fn new(net: Arc<dyn LauncherNet>) -> Self {
        Self {
            net,
            join_ticket: String::new(),
            join_dialog: None,
            console: None,
            lobby_failure: None,
            ended_lobby: None,
            session_options: SessionOptions::default(),
            host_result: None,
            join_result: None,
            host_task: None,
            join_task: None,
            abandoned: false,
            closing: false,
        }
    }

    pub fn poll(&mut self) {
        if let Some(result) = self.host_task.as_ref().and_then(Task::try_take) {
            self.host_task = None;
            self.host_result = Some(result);
            // The network answers in order, so the session asked to close before this host is gone.
            self.closing = false;
        }
        if let Some(result) = self.join_task.as_ref().and_then(Task::try_take) {
            self.join_task = None;
            self.join_result = Some(result);
            self.closing = false;
        }
        if self.closing
            && matches!(
                self.net.status().state,
                NetState::Idle | NetState::Failed(_)
            )
        {
            self.closing = false;
        }
        // A host or join given up on may still open a session when its call returns; close it then.
        if self.abandoned && !self.hosting_pending() && !self.joining_pending() {
            self.abandoned = false;
            self.give_up();
        }
    }

    /// Whether a session this launcher asked to close may still report itself open: the network can take
    /// seconds to let go of it.
    pub fn closing_session(&self) -> bool {
        self.closing
    }

    pub fn hosting_pending(&self) -> bool {
        self.host_task.is_some()
    }

    pub fn joining_pending(&self) -> bool {
        self.join_task.is_some()
    }

    pub fn start_host(&mut self, ctx: &Context, now_ms: f64) {
        let started_at = Instant::now();
        let net = Arc::clone(&self.net);
        self.host_result = None;
        self.lobby_failure = None;
        self.ended_lobby = None;
        self.net.set_session_options(self.session_options);
        self.host_task = Some(Task::spawn(move || net.host(), ctx.clone()));
        self.console = Some(ConsoleRun {
            task: ConsoleTask::Hosting,
            clock: ConsoleClock::new(now_ms),
            started_at,
        });
    }

    pub fn push_session_options(&self) {
        self.net.set_session_options(self.session_options);
    }

    pub fn start_join(&mut self, ctx: &Context, now_ms: f64) {
        let started_at = Instant::now();
        let net = Arc::clone(&self.net);
        let ticket = self.join_ticket.trim().to_owned();
        self.join_result = None;
        self.lobby_failure = None;
        self.ended_lobby = None;
        self.join_dialog = None;
        self.join_task = Some(Task::spawn(move || net.join(&ticket), ctx.clone()));
        self.console = Some(ConsoleRun {
            task: ConsoleTask::Joining,
            clock: ConsoleClock::new(now_ms),
            started_at,
        });
    }

    /// Drops a host or a join still under way, or one that failed, and the console that covered it.
    pub fn abandon(&mut self) {
        self.console = None;
        self.abandoned = self.hosting_pending() || self.joining_pending();
        self.give_up();
    }

    pub fn leave(&mut self) {
        self.net.leave();
        self.closing = true;
        self.host_result = None;
        self.join_result = None;
    }

    fn give_up(&mut self) {
        self.net.give_up();
        self.closing = true;
        self.host_result = None;
        self.join_result = None;
    }
}
