use std::path::{Path, PathBuf};

use nc_launcher_core::{
    LauncherEvent, LauncherNet, NetState, seat_error_words, seat_refusal_words,
};
use replicant_proto::CampaignCard;

use crate::saves::{
    GAMEDATA_SIZE, PreparedSession, keep_unsent, prepare_session_file, promote_delivered,
    read_session_file,
};
use crate::seat_round::{HOST_MEMBER_SLOT, shown_campaign_id};

/// The guest's side of the seat round: the session file built from the host's seat, and every
/// save of it sent back to the host.
pub struct GuestSeatRound {
    save_folder: Option<PathBuf>,
    template_sources: Vec<PathBuf>,
    backups: PathBuf,
    card: Option<CampaignCard>,
    prepared: Option<PreparedSession>,
    pub log: Vec<String>,
}

impl GuestSeatRound {
    pub fn new(
        save_folder: Option<PathBuf>,
        template_sources: Vec<PathBuf>,
        backups: PathBuf,
    ) -> Self {
        Self {
            save_folder,
            template_sources,
            backups,
            card: None,
            prepared: None,
            log: Vec::new(),
        }
    }

    pub fn save_folder(&self) -> Option<&Path> {
        self.save_folder.as_deref()
    }

    pub fn card(&self) -> Option<&CampaignCard> {
        self.card.as_ref()
    }

    pub fn seat_expected(&self) -> bool {
        self.card
            .as_ref()
            .is_some_and(|card| card.second_seat.is_some())
    }

    /// The session file the game boots into, once the host's seat has been written into it.
    pub fn session_file_name(&self) -> Option<&str> {
        let prepared = self.prepared.as_ref()?;
        let card = self.card.as_ref()?;
        (prepared.sync.campaign_id() == card.campaign_id).then_some(prepared.file_name.as_str())
    }

    pub fn prepared(&self) -> Option<&PreparedSession> {
        self.prepared.as_ref()
    }

    pub fn on_event(&mut self, net: &dyn LauncherNet, event: &LauncherEvent) {
        match event {
            LauncherEvent::CampaignCardReceived { card } => self.card = Some(card.clone()),
            LauncherEvent::SeatReceived { slot, seat, .. } if *slot == HOST_MEMBER_SLOT => {
                self.on_seat_from_host(net, seat);
            }
            LauncherEvent::GameSaved { succeeded: true } => self.send_changes(net),
            LauncherEvent::SeatDelivered { slot, transfer } if *slot == HOST_MEMBER_SLOT => {
                self.on_delivered(*transfer);
            }
            LauncherEvent::SeatTransferRefused {
                slot,
                transfer,
                reason,
                ..
            } if *slot == HOST_MEMBER_SLOT => {
                if let Some(prepared) = self.prepared.as_mut() {
                    prepared.sync.on_refused(*transfer);
                    self.log.push(format!(
                        "your save did not reach the host ({}); it is kept and sent again at the next save or session",
                        seat_refusal_words(*reason)
                    ));
                }
            }
            LauncherEvent::MemberLeft { slot, .. } if *slot == HOST_MEMBER_SLOT => self.link_lost(),
            LauncherEvent::Failed { .. } | LauncherEvent::Stopped => self.link_lost(),
            _ => {}
        }
    }

    /// The game ended or the lobby is closing: one more send if the file changed since the host
    /// last confirmed a save.
    pub fn flush(&mut self, net: &dyn LauncherNet) {
        self.send_changes(net);
    }

    fn on_seat_from_host(&mut self, net: &dyn LauncherNet, seat: &[u8]) {
        let Some(card) = self.card.clone() else {
            self.log.push(
                "a save arrived from the host before its campaign card; it was not used".to_owned(),
            );
            return;
        };
        let Some(folder) = self.save_folder.clone() else {
            self.log.push(
                "the host's save arrived but this machine has no save folder for it".to_owned(),
            );
            return;
        };
        let Some(template) = self.template() else {
            self.log.push(
                "the host's save arrived but there is no save file on this machine to build the session file from; start the game once"
                    .to_owned(),
            );
            return;
        };
        match prepare_session_file(&folder, &template, &self.backups, &card.campaign_id, seat) {
            Ok(prepared) => {
                self.log.push(format!(
                    "campaign {}: wrote {} with {}",
                    shown_campaign_id(&card.campaign_id),
                    prepared.path.display(),
                    if prepared.plan.send_back {
                        "this machine's newer save, which goes back to the host"
                    } else {
                        "the host's save"
                    }
                ));
                for path in &prepared.kept_aside {
                    self.log.push(format!(
                        "an older save from the previous session file was kept at {}",
                        path.display()
                    ));
                }
                self.prepared = Some(prepared);
                self.send_pending(net);
            }
            Err(reason) => self.log.push(format!(
                "campaign {}: the session file was not written: {reason}",
                shown_campaign_id(&card.campaign_id)
            )),
        }
    }

    fn send_changes(&mut self, net: &dyn LauncherNet) {
        let Some(prepared) = self.prepared.as_mut() else {
            return;
        };
        let file = match read_session_file(&prepared.path) {
            Ok(file) => file,
            Err(error) => {
                self.log.push(format!(
                    "could not read the session file after a save: {error}"
                ));
                return;
            }
        };
        if let Err(error) = prepared.sync.on_saved(&file) {
            self.log.push(format!(
                "the saved slot failed its checks ({}); nothing was sent and the file is untouched",
                seat_error_words(error)
            ));
            return;
        }
        self.send_pending(net);
    }

    fn send_pending(&mut self, net: &dyn LauncherNet) {
        let Some(prepared) = self.prepared.as_mut() else {
            return;
        };
        let Some(seat) = prepared.sync.to_send() else {
            return;
        };
        let id = prepared.sync.campaign_id().to_owned();
        if let Err(error) = keep_unsent(&self.backups, &id, &seat) {
            self.log.push(format!(
                "could not keep a copy of the save before sending it: {error}"
            ));
        }
        if !matches!(net.status().state, NetState::Connected { .. }) {
            self.log.push(format!(
                "campaign {}: not connected to the host, your save is kept and sent at the next session",
                shown_campaign_id(&id)
            ));
            return;
        }
        match net.send_seat(HOST_MEMBER_SLOT, seat.clone()) {
            Some(transfer) => {
                prepared.sync.sending(transfer, seat);
                self.log.push(format!(
                    "campaign {}: sending your save to the host (transfer {transfer})",
                    shown_campaign_id(&id)
                ));
            }
            None => self.log.push(
                "the network is not running: your save is kept and sent at the next session"
                    .to_owned(),
            ),
        }
    }

    fn on_delivered(&mut self, transfer: u32) {
        let Some(prepared) = self.prepared.as_mut() else {
            return;
        };
        let Some(seat) = prepared.sync.on_delivered(transfer) else {
            return;
        };
        let id = prepared.sync.campaign_id().to_owned();
        self.log.push(match promote_delivered(&self.backups, &id, &seat) {
            Ok(()) => format!("campaign {}: the host has your save", shown_campaign_id(&id)),
            Err(error) => format!(
                "campaign {}: the host has your save, but the hidden copy could not be updated: {error}",
                shown_campaign_id(&id)
            ),
        });
    }

    fn link_lost(&mut self) {
        self.card = None;
        if let Some(prepared) = self.prepared.as_mut() {
            prepared.sync.link_lost();
        }
    }

    fn template(&self) -> Option<Vec<u8>> {
        self.template_sources
            .iter()
            .filter_map(|path| std::fs::read(path).ok())
            .find(|bytes| bytes.len() == GAMEDATA_SIZE)
    }
}
