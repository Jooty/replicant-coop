use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use nc_net::Transport;

use crate::net::MAX_RELAYED_INBOX;
use crate::net::link_peer::LinkPeer;
use crate::net::net_game::{NetGame, NetSharedOf, SharedNetOf};
use crate::net::net_shared::NetShared;
use crate::net::worker::Worker;
use crate::relay::RelayInbox;

pub struct NetThread<G: NetGame> {
    snapshots: Sender<G::Snapshot>,
    relay_outbox: Sender<G::Message>,
    launcher_outbox: Sender<G::Message>,
    relay_inbox: Arc<RelayInbox<G::Message>>,
    shared: SharedNetOf<G>,
    wants_snapshots: Arc<AtomicBool>,
}

impl<G: NetGame> NetThread<G> {
    pub fn spawn<T>(game: G, transport: T, link: LinkPeer) -> io::Result<Self>
    where
        T: Transport + 'static,
    {
        let (snapshots, snapshot_inbox) = channel();
        let (relay_outbox, relay_queue) = channel();
        let (launcher_outbox, launcher_queue) = channel();
        let relay_inbox = Arc::new(RelayInbox::new(MAX_RELAYED_INBOX));
        let shared: SharedNetOf<G> = Arc::new(Mutex::new(NetShared::default()));
        let wants_snapshots = Arc::new(AtomicBool::new(false));
        let worker = Worker::new(
            game,
            transport,
            link,
            Arc::clone(&shared),
            Arc::clone(&wants_snapshots),
            Arc::clone(&relay_inbox),
        );
        thread::Builder::new()
            .name("nc-net".to_owned())
            .spawn(move || worker.run(&snapshot_inbox, &relay_queue, &launcher_queue))?;
        Ok(Self {
            snapshots,
            relay_outbox,
            launcher_outbox,
            relay_inbox,
            shared,
            wants_snapshots,
        })
    }

    pub fn publish(&self, snapshot: G::Snapshot) {
        self.update(|shared| shared.local = Some(snapshot));
        let _ = self.snapshots.send(snapshot);
    }

    /// Queues a game message for the other player; dropped unless another player is in play.
    pub fn send_relayed(&self, message: G::Message) {
        let _ = self.relay_outbox.send(message);
    }

    /// Removes and returns the waiting game messages `wanted` accepts, oldest first; others stay queued.
    pub fn take_relayed(&self, wanted: impl Fn(&G::Message) -> bool) -> Vec<G::Message> {
        self.relay_inbox.take_matching(wanted)
    }

    /// As [`Self::take_relayed`], each message with the slot of the player who sent it.
    pub fn take_relayed_from(&self, wanted: impl Fn(&G::Message) -> bool) -> Vec<(u8, G::Message)> {
        self.relay_inbox
            .take_matching_from(wanted)
            .into_iter()
            .map(|relayed| (relayed.from, relayed.message))
            .collect()
    }

    pub fn send_to_launcher(&self, message: G::Message) {
        let _ = self.launcher_outbox.send(message);
    }

    pub fn update<R>(&self, change: impl FnOnce(&mut NetSharedOf<G>) -> R) -> R {
        change(&mut self.shared.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub fn take_quit_request(&self) -> bool {
        self.update(|shared| std::mem::take(&mut shared.quit_requested))
    }

    pub fn wants_snapshots(&self) -> bool {
        self.wants_snapshots.load(Ordering::Relaxed)
    }

    pub fn view(&self) -> NetSharedOf<G> {
        self.update(|shared| shared.clone())
    }
}
