use nc_net::ProgressEntry;
use nc_wire::SessionRules;
use replicant_proto::{CampaignCard, LobbyReady, PartnerBody, PlayerSettings, SessionOptions};

use crate::{ConnectionReport, NetError, NetStatus};

pub trait LauncherNet: Send + Sync {
    fn host(&self) -> Result<String, NetError>;
    fn join(&self, ticket: &str) -> Result<(), NetError>;
    fn test_connection(&self, ticket: &str, seconds: u32) -> Result<ConnectionReport, NetError>;
    /// The player's own Leave or Close lobby.
    fn leave(&self);
    /// Drops a host or a join still under way, which the player gave up on.
    fn give_up(&self) {
        self.leave();
    }
    fn set_session_options(&self, _options: SessionOptions) {}
    fn set_player_settings(&self, _settings: PlayerSettings) {}
    fn send_lobby_ready(&self, _ready: LobbyReady) {}
    fn send_lobby_body(&self, _body: PartnerBody) {}
    fn start_lobby(&self, _mode: u8) {}
    fn set_rules(&self, _rules: SessionRules) {}
    fn send_campaign_card(&self, _card: CampaignCard) {}
    /// Sends a seat to one member and returns the transfer number its receipt will carry.
    fn send_seat(&self, _slot: u8, _seat: Vec<u8>) -> Option<u32> {
        None
    }
    fn ask_game_to_quit(&self) {}
    /// What the network has done so far for the session being set up, oldest first.
    fn progress(&self) -> Vec<ProgressEntry> {
        Vec::new()
    }
    fn status(&self) -> NetStatus;
}
