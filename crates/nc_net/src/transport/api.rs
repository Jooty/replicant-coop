use crate::progress::ProgressSink;
use crate::transport::{Channel, Invite, LinkStats, NetError, PeerId, TransportEvent};

pub trait Transport: Send {
    fn local_peer(&self) -> PeerId;
    fn host(&mut self) -> Result<Invite, NetError>;
    fn join(&mut self, invite: &Invite) -> Result<(), NetError>;
    fn send(&mut self, to: PeerId, channel: Channel, payload: &[u8]) -> Result<(), NetError>;
    fn poll(&mut self, out: &mut Vec<TransportEvent>);
    fn link_stats(&self, peer: PeerId) -> Option<LinkStats>;
    fn disconnect(&mut self, peer: PeerId);
    /// Sends what the link does while it is set up to `sink`; a transport that reports nothing ignores it.
    fn report_progress(&mut self, _sink: ProgressSink) {}
}

impl Transport for Box<dyn Transport> {
    fn local_peer(&self) -> PeerId {
        (**self).local_peer()
    }

    fn host(&mut self) -> Result<Invite, NetError> {
        (**self).host()
    }

    fn join(&mut self, invite: &Invite) -> Result<(), NetError> {
        (**self).join(invite)
    }

    fn send(&mut self, to: PeerId, channel: Channel, payload: &[u8]) -> Result<(), NetError> {
        (**self).send(to, channel, payload)
    }

    fn poll(&mut self, out: &mut Vec<TransportEvent>) {
        (**self).poll(out);
    }

    fn link_stats(&self, peer: PeerId) -> Option<LinkStats> {
        (**self).link_stats(peer)
    }

    fn disconnect(&mut self, peer: PeerId) {
        (**self).disconnect(peer);
    }

    fn report_progress(&mut self, sink: ProgressSink) {
        (**self).report_progress(sink);
    }
}
