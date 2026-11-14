use nc_wire::{Hello, MAX_MESSAGE_BYTES, Reject, RejectReason, WireError, decode, encode};

use crate::tiny::Tiny;

#[test]
fn a_message_round_trips() {
    let message = Tiny::Blob(vec![1, 2, 3]);
    let bytes = encode(&message).expect("encodes");
    assert_eq!(decode::<Tiny>(&bytes), Ok(message));
}

#[test]
fn an_oversize_message_is_refused_both_ways() {
    let message = Tiny::Blob(vec![0; MAX_MESSAGE_BYTES]);
    assert!(matches!(encode(&message), Err(WireError::TooLarge { .. })));
    assert!(matches!(
        decode::<Tiny>(&vec![0; MAX_MESSAGE_BYTES + 1]),
        Err(WireError::TooLarge { .. })
    ));
}

#[test]
fn empty_trailing_and_invalid_input_is_refused() {
    assert_eq!(decode::<Tiny>(&[]), Err(WireError::Empty));
    let mut bytes = encode(&Tiny::Blob(vec![9])).expect("encodes");
    bytes.push(0);
    assert_eq!(
        decode::<Tiny>(&bytes),
        Err(WireError::TrailingBytes { extra: 1 })
    );
    let forbidden = postcard::to_stdvec(&Tiny::Forbidden).expect("serializes");
    assert!(decode::<Tiny>(&forbidden).is_err());
    assert!(encode(&Tiny::Forbidden).is_err());
}

#[test]
fn link_payloads_keep_their_encoded_shape() {
    let reject = Reject {
        reason: RejectReason::BuildMismatch,
    };
    assert_eq!(postcard::to_stdvec(&reject).expect("serializes"), vec![1]);
    let hello = Hello {
        version: 300,
        build: nc_wire::BuildGate {
            time_date_stamp: 1,
            size_of_image: 2,
        },
        name: "A".to_owned(),
    };
    assert_eq!(
        postcard::to_stdvec(&hello).expect("serializes"),
        vec![0xAC, 0x02, 1, 2, 1, b'A']
    );
}
