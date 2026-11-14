use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use data_encoding::BASE32_NOPAD;
use iroh::{EndpointAddr, EndpointId};
use nc_net::{
    Channel, Invite, IrohConfig, IrohTransport, LinkProgress, LookupMeans, PathRoute,
    ProgressBoard, Transport, TransportEvent,
};

const WAIT_LIMIT: Duration = Duration::from_secs(20);
const POLL_STEP: Duration = Duration::from_millis(20);
const KEY_TICKET_PREFIX: &str = "rcn2";
const KEY_TICKET_LENGTH: usize = 56;
const BOTH_CHANNELS_LOOKUP_PORT: u16 = 47_551;
const READS_LOOKUP_PORT: u16 = 47_552;
const PROGRESS_LOOKUP_PORT: u16 = 47_553;
const KEY_ONLY_LOOKUP_PORT: u16 = 47_554;
const OLDER_TICKET_HOST_LOOKUP_PORT: u16 = 47_555;
const OLDER_TICKET_GUEST_LOOKUP_PORT: u16 = 47_556;
const NOT_FOUND_HOST_LOOKUP_PORT: u16 = 47_557;
const NOT_FOUND_GUEST_LOOKUP_PORT: u16 = 47_558;
const REFUSED_LOOKUP_PORT: u16 = 47_559;
const KEY_JOIN_LOOKUP_PORT: u16 = 47_560;

fn local_transport(local_lookup_port: u16) -> IrohTransport {
    IrohTransport::new(IrohConfig {
        use_relays_and_lookup: false,
        local_lookup_port,
    })
    .expect("endpoint binds")
}

fn connected(events: &[TransportEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, TransportEvent::PeerConnected(_)))
}

fn told(board: &ProgressBoard) -> Vec<LinkProgress> {
    board.entries().into_iter().map(|e| e.progress).collect()
}

fn older_form_ticket(addr: &EndpointAddr) -> Invite {
    let raw = postcard::to_stdvec(addr).expect("address encodes");
    let body = BASE32_NOPAD.encode(&raw).to_ascii_lowercase();
    Invite::new(format!("rcn1{body}")).expect("fits")
}

fn wait_until(
    host: &mut IrohTransport,
    guest: &mut IrohTransport,
    host_events: &mut Vec<TransportEvent>,
    guest_events: &mut Vec<TransportEvent>,
    done: impl Fn(&[TransportEvent], &[TransportEvent]) -> bool,
) {
    let deadline = Instant::now() + WAIT_LIMIT;
    while Instant::now() < deadline {
        host.poll(host_events);
        guest.poll(guest_events);
        if done(host_events, guest_events) {
            return;
        }
        std::thread::sleep(POLL_STEP);
    }
    panic!("timed out; host saw {host_events:?}, guest saw {guest_events:?}");
}

fn received(events: &[TransportEvent], channel: Channel, expected: &[u8]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            TransportEvent::Message { channel: seen, payload, .. }
                if *seen == channel && payload == expected
        )
    })
}

#[test]
fn two_local_endpoints_connect_and_exchange_both_channels() {
    let mut host = local_transport(BOTH_CHANNELS_LOOKUP_PORT);
    let mut guest = local_transport(BOTH_CHANNELS_LOOKUP_PORT);
    let invite = host.host().expect("host issues an invite");
    guest.join(&invite).expect("guest dials");
    let (mut host_events, mut guest_events) = (Vec::new(), Vec::new());
    wait_until(
        &mut host,
        &mut guest,
        &mut host_events,
        &mut guest_events,
        |host_seen, guest_seen| connected(host_seen) && connected(guest_seen),
    );
    let guest_id = guest.local_peer();
    host.send(guest_id, Channel::ReliableOrdered, b"reliable hello")
        .expect("reliable send");
    for _ in 0..50 {
        host.send(guest_id, Channel::UnreliableLatest, b"datagram hello")
            .expect("datagram send");
    }
    wait_until(
        &mut host,
        &mut guest,
        &mut host_events,
        &mut guest_events,
        |_, guest_seen| {
            received(guest_seen, Channel::ReliableOrdered, b"reliable hello")
                && received(guest_seen, Channel::UnreliableLatest, b"datagram hello")
        },
    );
    let stats = host.link_stats(guest_id).expect("link has stats");
    assert!(stats.direct);
}

#[test]
fn an_invite_that_is_not_ours_is_refused() {
    let mut guest = local_transport(REFUSED_LOOKUP_PORT);
    let invite = nc_net::Invite::new("rcn1notbase32!".to_owned()).expect("valid text");
    assert_eq!(guest.join(&invite), Err(nc_net::NetError::InvalidInvite));
}

#[test]
fn only_an_invite_this_transport_wrote_reads_without_connecting() {
    let mut host = local_transport(READS_LOOKUP_PORT);
    let invite = host.host().expect("invite");
    assert!(nc_net::invite_reads(invite.as_str()));
    assert!(nc_net::invite_reads(
        older_form_ticket(&EndpointAddr::new(key_of(&host))).as_str()
    ));
    assert!(!nc_net::invite_reads("rcn1notbase32!"));
    assert!(!nc_net::invite_reads("loopback-1"));
    assert!(!nc_net::invite_reads(""));
}

#[test]
fn a_local_join_reports_the_link_as_it_comes_up() {
    let mut host = local_transport(PROGRESS_LOOKUP_PORT);
    let mut guest = local_transport(PROGRESS_LOOKUP_PORT);
    let host_board = ProgressBoard::default();
    let guest_board = ProgressBoard::default();
    host.report_progress(host_board.fresh_sink());
    guest.report_progress(guest_board.fresh_sink());
    let invite = host.host().expect("host issues an invite");
    guest.join(&invite).expect("guest dials");
    let (mut host_events, mut guest_events) = (Vec::new(), Vec::new());
    wait_until(
        &mut host,
        &mut guest,
        &mut host_events,
        &mut guest_events,
        |_, _| {
            guest_board
                .entries()
                .iter()
                .any(|entry| matches!(entry.progress, LinkProgress::PathWon { .. }))
        },
    );
    let host_told = told(&host_board);
    assert!(matches!(
        host_told[0],
        LinkProgress::Bound {
            ipv4_port: Some(_),
            ..
        }
    ));
    assert!(matches!(host_told[1], LinkProgress::OwnId(_)));
    assert!(
        host_told.contains(&LinkProgress::LocalLookupAnswering {
            port: PROGRESS_LOOKUP_PORT
        }),
        "{host_told:?}"
    );
    let guest_told = told(&guest_board);
    let place = |wanted: fn(&LinkProgress) -> bool| {
        guest_told
            .iter()
            .position(wanted)
            .unwrap_or_else(|| panic!("missing from {guest_told:?}"))
    };
    let read = place(|p| {
        matches!(
            p,
            LinkProgress::TicketRead {
                relay: None,
                direct_addresses: 0,
                ..
            }
        )
    });
    let looking = place(|p| matches!(p, LinkProgress::LookingUpHost));
    let found = place(|p| {
        matches!(
            p,
            LinkProgress::HostFound {
                by: LookupMeans::LocalNetwork,
                direct_addresses: 1..,
                ..
            }
        )
    });
    let answered = place(|p| matches!(p, LinkProgress::HostAnswered { .. }));
    let won = place(|p| {
        matches!(
            p,
            LinkProgress::PathWon {
                route: PathRoute::Direct(_),
                ..
            }
        )
    });
    assert!(
        read < looking && looking < found && found < answered && answered < won,
        "{guest_told:?}"
    );
}

fn key_of(transport: &IrohTransport) -> EndpointId {
    EndpointId::from_bytes(&transport.local_peer().0).expect("a transport's id is a key")
}

fn join_and_wait(host: &mut IrohTransport, guest: &mut IrohTransport, invite: &Invite) {
    guest.join(invite).expect("guest looks the host up");
    let (mut host_events, mut guest_events) = (Vec::new(), Vec::new());
    wait_until(
        host,
        guest,
        &mut host_events,
        &mut guest_events,
        |host_seen, guest_seen| connected(host_seen) && connected(guest_seen),
    );
}

#[test]
fn a_new_ticket_is_the_prefix_and_the_host_key_alone() {
    let mut host = local_transport(KEY_ONLY_LOOKUP_PORT);
    let invite = host.host().expect("invite");
    let text = invite.as_str();
    assert_eq!(text.len(), KEY_TICKET_LENGTH);
    let body = text.strip_prefix(KEY_TICKET_PREFIX).expect("our prefix");
    let key = BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .expect("base32");
    assert_eq!(key, host.local_peer().0);
}

#[test]
fn a_join_by_the_key_alone_finds_the_host_over_the_local_lookup() {
    let mut host = local_transport(KEY_JOIN_LOOKUP_PORT);
    let mut guest = local_transport(KEY_JOIN_LOOKUP_PORT);
    let invite = host.host().expect("invite");
    join_and_wait(&mut host, &mut guest, &invite);
    assert_eq!(
        host.link_stats(guest.local_peer())
            .map(|stats| stats.direct),
        Some(true)
    );
}

#[test]
fn an_older_ticket_still_joins_at_the_addresses_it_carries() {
    let mut host = local_transport(OLDER_TICKET_HOST_LOOKUP_PORT);
    let mut guest = local_transport(OLDER_TICKET_GUEST_LOOKUP_PORT);
    let host_board = ProgressBoard::default();
    let guest_board = ProgressBoard::default();
    host.report_progress(host_board.fresh_sink());
    guest.report_progress(guest_board.fresh_sink());
    let Some(LinkProgress::Bound {
        ipv4_port: Some(port),
        ..
    }) = told(&host_board).into_iter().next()
    else {
        panic!("the host told its port first: {:?}", told(&host_board));
    };
    let addr = EndpointAddr::new(key_of(&host))
        .with_ip_addr(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port));
    join_and_wait(&mut host, &mut guest, &older_form_ticket(&addr));
    let guest_told = told(&guest_board);
    assert!(
        guest_told.iter().any(|p| matches!(
            p,
            LinkProgress::HostFound {
                by: LookupMeans::TicketAddresses,
                ..
            }
        )),
        "{guest_told:?}"
    );
}

#[test]
fn a_key_nobody_answers_for_is_not_found_and_says_why() {
    let mut host = local_transport(NOT_FOUND_HOST_LOOKUP_PORT);
    let mut guest = local_transport(NOT_FOUND_GUEST_LOOKUP_PORT);
    let guest_board = ProgressBoard::default();
    guest.report_progress(guest_board.fresh_sink());
    let invite = host.host().expect("invite");
    guest.join(&invite).expect("the ticket reads");
    let deadline = Instant::now() + WAIT_LIMIT;
    let reason = loop {
        let found = told(&guest_board).into_iter().find_map(|p| match p {
            LinkProgress::HostNotFound { reason } => Some(reason),
            _ => None,
        });
        if let Some(reason) = found {
            break reason;
        }
        assert!(Instant::now() < deadline, "{:?}", told(&guest_board));
        std::thread::sleep(POLL_STEP);
    };
    assert!(reason.contains("local network"), "{reason}");
    let mut guest_events = Vec::new();
    guest.poll(&mut guest_events);
    assert!(!connected(&guest_events));
}
