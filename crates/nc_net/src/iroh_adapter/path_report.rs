use std::collections::BTreeSet;
use std::pin::pin;

use futures_lite::StreamExt;
use iroh::TransportAddr;
use iroh::endpoint::{Connection, PathEvent, PathId};

use crate::progress::{AddressKind, LinkProgress, PathRoute, ProgressSink};

#[derive(Default)]
struct PathsSeen {
    tried: BTreeSet<PathId>,
    won: Option<PathId>,
}

impl PathsSeen {
    fn tried(&mut self, id: PathId, remote: &TransportAddr, sink: &ProgressSink) {
        if let Some(route) = route_of(remote)
            && self.tried.insert(id)
        {
            sink.send(LinkProgress::PathTried(route));
        }
    }

    fn won(
        &mut self,
        connection: &Connection,
        id: PathId,
        remote: &TransportAddr,
        sink: &ProgressSink,
    ) {
        self.tried(id, remote, sink);
        if let Some(route) = route_of(remote)
            && self.won != Some(id)
        {
            self.won = Some(id);
            sink.send(LinkProgress::PathWon {
                route,
                rtt: connection.rtt(id),
            });
        }
    }
}

pub(super) async fn report_paths(connection: Connection, sink: ProgressSink) {
    let mut events = pin!(connection.path_events());
    let mut seen = PathsSeen::default();
    // Paths opened before the stream was taken are only in the snapshot.
    let open: Vec<(PathId, TransportAddr, bool)> = connection
        .paths()
        .iter()
        .map(|path| (path.id(), path.remote_addr().clone(), path.is_selected()))
        .collect();
    for (id, remote, selected) in &open {
        if *selected {
            seen.won(&connection, *id, remote, &sink);
        } else {
            seen.tried(*id, remote, &sink);
        }
    }
    while let Some(event) = events.next().await {
        match event {
            PathEvent::Opened {
                id, remote_addr, ..
            } => seen.tried(id, &remote_addr, &sink),
            PathEvent::Selected {
                id, remote_addr, ..
            } => seen.won(&connection, id, &remote_addr, &sink),
            _ => {}
        }
    }
}

fn route_of(remote: &TransportAddr) -> Option<PathRoute> {
    match remote {
        TransportAddr::Ip(addr) => Some(PathRoute::Direct(AddressKind::of(addr.ip()))),
        TransportAddr::Relay(_) => Some(PathRoute::Relayed),
        _ => None,
    }
}
