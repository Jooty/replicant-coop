use std::io;
use std::ops::Deref;
use std::time::Instant;

use nc_hook::net::LinkPeer;
use nc_net::Transport;
use replicant_proto::{GameReadiness, Message, PlayerSettings, SessionOptions};

use crate::net::replicant_net::ReplicantNet;

pub struct NetThread {
    inner: nc_hook::net::NetThread<ReplicantNet>,
}

impl Deref for NetThread {
    type Target = nc_hook::net::NetThread<ReplicantNet>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl NetThread {
    pub fn spawn<T>(transport: T, link_port: u16) -> io::Result<Self>
    where
        T: Transport + 'static,
    {
        nc_hook::net::NetThread::spawn(
            ReplicantNet::default(),
            transport,
            LinkPeer::launcher_on_port(link_port),
        )
        .map(|inner| Self { inner })
    }

    pub fn take_launcher_settings(&self) -> Option<PlayerSettings> {
        self.inner.update(|shared| shared.pending_settings.take())
    }

    pub fn tell_launcher_settings(&self, settings: PlayerSettings) {
        self.inner.send_to_launcher(Message::Settings(settings));
    }

    pub fn tell_launcher_host_options(&self, options: SessionOptions) {
        self.inner.update(|shared| shared.options = options);
        self.inner
            .send_to_launcher(Message::SessionOptions(options));
    }

    pub fn tell_launcher_saved(&self, succeeded: bool) {
        self.inner
            .send_to_launcher(Message::GameSaved { succeeded });
    }

    pub fn report_readiness(&self, readiness: GameReadiness) {
        self.inner.update(|shared| {
            shared.local_readiness = readiness;
            shared.local_readiness_at = Some(Instant::now());
        });
    }
}
