use nc_hook::relay::{RelayInbox, Relayed};

const HOST_SLOT: u8 = 0;
const GUEST_SLOT: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
enum MadeUpMessage {
    Wave(u8),
    Chat(&'static str),
}

fn is_wave(message: &MadeUpMessage) -> bool {
    matches!(message, MadeUpMessage::Wave(_))
}

#[test]
fn taking_matching_messages_leaves_the_others_queued_in_order() {
    let inbox = RelayInbox::new(8);
    inbox.push(HOST_SLOT, MadeUpMessage::Wave(1));
    inbox.push(HOST_SLOT, MadeUpMessage::Chat("hi"));
    inbox.push(HOST_SLOT, MadeUpMessage::Wave(2));
    assert_eq!(
        inbox.take_matching(is_wave),
        vec![MadeUpMessage::Wave(1), MadeUpMessage::Wave(2)]
    );
    assert_eq!(
        inbox.take_matching(|_| true),
        vec![MadeUpMessage::Chat("hi")]
    );
}

#[test]
fn a_full_inbox_drops_its_oldest_message() {
    let inbox = RelayInbox::new(2);
    inbox.push(HOST_SLOT, MadeUpMessage::Wave(1));
    inbox.push(HOST_SLOT, MadeUpMessage::Wave(2));
    inbox.push(HOST_SLOT, MadeUpMessage::Wave(3));
    assert_eq!(
        inbox.take_matching(is_wave),
        vec![MadeUpMessage::Wave(2), MadeUpMessage::Wave(3)]
    );
}

#[test]
fn clearing_forgets_everything() {
    let inbox = RelayInbox::new(4);
    inbox.push(HOST_SLOT, MadeUpMessage::Chat("a"));
    inbox.clear();
    assert!(inbox.is_empty());
    assert_eq!(inbox.len(), 0);
}

#[test]
fn take_matching_drops_the_sender_take_matching_from_keeps_it() {
    let inbox = RelayInbox::new(4);
    inbox.push(HOST_SLOT, MadeUpMessage::Wave(1));
    inbox.push(GUEST_SLOT, MadeUpMessage::Wave(2));
    inbox.push(GUEST_SLOT, MadeUpMessage::Chat("hi"));
    assert_eq!(
        inbox.take_matching(|message| *message == MadeUpMessage::Wave(1)),
        vec![MadeUpMessage::Wave(1)]
    );
    assert_eq!(
        inbox.take_matching_from(|_| true),
        vec![
            Relayed {
                from: GUEST_SLOT,
                message: MadeUpMessage::Wave(2)
            },
            Relayed {
                from: GUEST_SLOT,
                message: MadeUpMessage::Chat("hi")
            },
        ]
    );
}
