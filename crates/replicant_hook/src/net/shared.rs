use replicant_game::readiness::PartnerWatch;
use replicant_game::story_calls::{PartnerPlace, Whereabouts};
use replicant_proto::PlayerState;

use crate::net::link_state::ReplicantLinkState;
use crate::net::local_snapshot::LocalSnapshot;
use crate::net::{NetState, net_clock_ms};

pub type NetShared =
    nc_hook::net::NetShared<LocalSnapshot, PlayerState, PartnerWatch, ReplicantLinkState>;
pub type RemotePlayer = nc_hook::net::RemotePlayer<PlayerState>;
pub type SharedNet =
    nc_hook::net::SharedNet<LocalSnapshot, PlayerState, PartnerWatch, ReplicantLinkState>;

pub trait NetSharedExt {
    /// The partner's watch, or an empty one while no partner is in the session.
    fn partner_watch(&self) -> PartnerWatch;

    fn partner_in_play(&self) -> bool;

    fn partner_joining(&self) -> bool;

    /// Whether the partner, in play, stands where a shared scene on this game would include them.
    fn partner_place(&self) -> PartnerPlace;

    fn partner_counts_for_zone_rule(&self) -> bool;
}

impl NetSharedExt for NetShared {
    fn partner_watch(&self) -> PartnerWatch {
        self.partner()
            .map_or_else(PartnerWatch::new, |partner| partner.watch)
    }

    fn partner_in_play(&self) -> bool {
        self.state == NetState::Connected && self.partner_watch().is_in_play()
    }

    fn partner_joining(&self) -> bool {
        self.state == NetState::Connected && self.partner_watch().is_joining()
    }

    fn partner_place(&self) -> PartnerPlace {
        if !self.partner_in_play() {
            return PartnerPlace::Unknown;
        }
        PartnerPlace::judge(
            self.local.as_ref().map(|local| Whereabouts {
                phase: local.phase,
                position: local.position,
            }),
            self.partner_state().map(|remote| Whereabouts {
                phase: remote.state.phase,
                position: remote.state.position,
            }),
        )
    }

    fn partner_counts_for_zone_rule(&self) -> bool {
        self.state == NetState::Connected
            && self.partner_watch().counts_for_zone_rule(net_clock_ms())
    }
}
