use nc_launcher_core::{LinkStatus, Members};
use nc_net::PeerId;
use replicant_proto::Message;

const HOST_PEER: PeerId = PeerId([1; 32]);
const OTHER_PEER: PeerId = PeerId([2; 32]);

fn three_members() -> Members {
    let mut members = Members::default();
    members.admit(0, "Hosty".to_owned(), HOST_PEER);
    members.admit(2, "Second".to_owned(), HOST_PEER);
    members.admit(5, "Fifth".to_owned(), HOST_PEER);
    members
}

#[test]
fn members_are_held_by_slot() {
    let mut members = three_members();
    assert_eq!(members.count(), 3);
    assert!(members.is_member(2));
    assert!(!members.is_member(1));
    assert_eq!(members.first().map(|member| member.slot), Some(0));
    let slots: Vec<u8> = members.all().map(|member| member.slot).collect();
    assert_eq!(slots, vec![0, 2, 5]);
    assert!(!members.admit(2, "Renamed".to_owned(), OTHER_PEER));
    assert_eq!(members.count(), 3);
    assert_eq!(
        members.leave(2).map(|member| member.name),
        Some("Renamed".to_owned())
    );
    assert!(members.leave(2).is_none());
    assert_eq!(members.count(), 2);
}

#[test]
fn a_partner_exists_only_while_exactly_one_member_is_in() {
    let mut members = Members::default();
    assert!(members.partner().is_none());
    members.admit(1, "Guesty".to_owned(), OTHER_PEER);
    assert_eq!(members.partner().map(|member| member.slot), Some(1));
    members.admit(2, "Second".to_owned(), OTHER_PEER);
    assert!(members.partner().is_none());
}

#[test]
fn a_game_address_names_a_member_or_everyone() {
    let members = three_members();
    assert!(members.addresses_a_member(None));
    assert!(members.addresses_a_member(Some(5)));
    assert!(!members.addresses_a_member(Some(3)));
    assert!(!Members::default().addresses_a_member(None));
}

#[test]
fn every_member_has_an_announcement() {
    let members = three_members();
    for slot in [0, 2, 5] {
        assert!(matches!(
            members.member_announcement(slot),
            Some(Message::Member(member)) if member.slot == slot
        ));
    }
    assert!(members.member_announcement(1).is_none());
}

#[test]
fn the_status_names_the_partner_only_when_there_is_one() {
    let mut status = LinkStatus::default();
    let mut members = Members::default();
    members.admit(1, "Guesty".to_owned(), OTHER_PEER);
    members.write_status(&mut status);
    assert_eq!(
        status.partner().map(|partner| partner.name.as_str()),
        Some("Guesty")
    );
    members.admit(2, "Second".to_owned(), OTHER_PEER);
    members.write_status(&mut status);
    assert!(status.partner().is_none());
    assert_eq!(status.members.len(), 2);
}
