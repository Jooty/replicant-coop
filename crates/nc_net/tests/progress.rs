use std::net::IpAddr;
use std::time::Duration;

use nc_net::{AddressKind, AddressReach, LinkProgress, PathRoute, ProgressBoard};

fn reach_of(ip: &str) -> AddressReach {
    AddressKind::of(ip.parse::<IpAddr>().expect("an address")).reach
}

#[test]
fn a_fresh_sink_starts_the_board_over_and_silences_the_old_one() {
    let board = ProgressBoard::default();
    let first = board.fresh_sink();
    first.send(LinkProgress::OwnId("abc".to_owned()));
    assert_eq!(board.entries().len(), 1);
    let second = board.fresh_sink();
    assert!(board.entries().is_empty());
    first.send(LinkProgress::DialFailed);
    second.send(LinkProgress::HelloSent { protocol: 67 });
    let entries = board.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].progress,
        LinkProgress::HelloSent { protocol: 67 }
    );
}

#[test]
fn an_address_is_told_by_its_reach() {
    assert_eq!(reach_of("127.0.0.1"), AddressReach::ThisComputer);
    assert_eq!(reach_of("192.168.1.20"), AddressReach::LocalNetwork);
    assert_eq!(reach_of("10.4.0.3"), AddressReach::LocalNetwork);
    assert_eq!(reach_of("100.72.1.9"), AddressReach::LocalNetwork);
    assert_eq!(reach_of("81.2.69.160"), AddressReach::Public);
    assert_eq!(reach_of("fe80::1"), AddressReach::LocalNetwork);
    assert_eq!(reach_of("fd12::5"), AddressReach::LocalNetwork);
    assert_eq!(reach_of("2001:db8::7"), AddressReach::Public);
    assert!(AddressKind::of("::1".parse().expect("an address")).ipv6);
}

#[test]
fn no_progress_line_writes_an_address_out() {
    let kind = AddressKind::of("81.2.69.160".parse().expect("an address"));
    let lines = [
        LinkProgress::Bound {
            ipv4_port: Some(51820),
            ipv6_port: None,
        },
        LinkProgress::DirectAddressFound { kind, count: 2 },
        LinkProgress::PathTried(PathRoute::Direct(kind)),
        LinkProgress::PathWon {
            route: PathRoute::Relayed,
            rtt: Some(Duration::from_millis(31)),
        },
    ]
    .map(|progress| progress.describe());
    for line in lines {
        let addresses = line
            .split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
            .filter(|word| word.parse::<IpAddr>().is_ok())
            .count();
        assert_eq!(addresses, 0, "{line}");
        assert!(!line.contains("81.2"), "{line}");
    }
}
