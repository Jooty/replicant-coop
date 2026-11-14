use nc_wire::{Envelope, MAX_MESSAGE_BYTES, encode_enveloped};
use replicant_proto::{
    CampaignCard, MAX_CAMPAIGN_ID_BYTES, MAX_CARD_NAME_BYTES, Message, PROTOCOL_VERSION,
    PartnerBody, SEAT_PART_BYTES, SaveSummary, SeatOutcome, SeatPart, SeatReceipt, SeatRefusal,
    SecondSeatCard, decode, encode, parts_for,
};

const FIRST_SEAT_VARIANT_TAG: u8 = 109;
const SEAT_SIZE_BYTES: u32 = 0x9260;

fn summary(name: &str) -> SaveSummary {
    SaveSummary {
        name: name.to_owned(),
        level: 12,
        play_seconds: 3600.5,
    }
}

fn card(second_seat: Option<SecondSeatCard>) -> CampaignCard {
    CampaignCard {
        campaign_id: "0123456789abcdef0123456789abcdef".to_owned(),
        host: summary("Nier"),
        second_seat,
    }
}

fn part(transfer: u32, part: u16, bytes: usize) -> SeatPart {
    SeatPart {
        transfer,
        part,
        parts: u16::try_from(parts_for(SEAT_SIZE_BYTES)).expect("fits"),
        total_bytes: SEAT_SIZE_BYTES,
        checksum: 7,
        bytes: vec![1; bytes],
    }
}

#[test]
fn the_seat_messages_are_part_of_the_current_protocol() {
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn the_seat_messages_come_after_every_existing_variant() {
    let tags: Vec<u8> = [
        Message::CampaignCard(card(None)),
        Message::SeatPart(part(1, 0, SEAT_PART_BYTES)),
        Message::SeatReceipt(SeatReceipt {
            transfer: 1,
            outcome: SeatOutcome::ReceivedWholeAndValid,
        }),
    ]
    .iter()
    .map(|message| encode(message).expect("encodes")[0])
    .collect();
    assert_eq!(
        tags,
        vec![
            FIRST_SEAT_VARIANT_TAG,
            FIRST_SEAT_VARIANT_TAG + 1,
            FIRST_SEAT_VARIANT_TAG + 2
        ]
    );
}

#[test]
fn a_campaign_card_with_and_without_a_second_seat_round_trips() {
    let seated = SecondSeatCard {
        summary: summary("Kaine"),
        body: Some(PartnerBody::PrologueCharacter),
    };
    for message in [
        Message::CampaignCard(card(None)),
        Message::CampaignCard(card(Some(seated))),
    ] {
        assert!(message.is_relayed());
        let bytes = encode(&message).expect("encodes");
        assert_eq!(decode(&bytes), Ok(message));
    }
}

#[test]
fn the_largest_card_fits_one_message() {
    let longest = "x".repeat(MAX_CARD_NAME_BYTES);
    let big = CampaignCard {
        campaign_id: "i".repeat(MAX_CAMPAIGN_ID_BYTES),
        host: SaveSummary {
            name: longest.clone(),
            level: i32::MAX,
            play_seconds: f64::MAX,
        },
        second_seat: Some(SecondSeatCard {
            summary: SaveSummary {
                name: longest,
                level: i32::MAX,
                play_seconds: f64::MAX,
            },
            body: Some(PartnerBody::SeedKaineE),
        }),
    };
    let bytes = encode(&Message::CampaignCard(big)).expect("encodes");
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
}

#[test]
fn the_largest_seat_part_fits_one_message() {
    let mut worst = part(u32::MAX, 0, SEAT_PART_BYTES);
    worst.checksum = u32::MAX;
    let envelope = Envelope {
        from: 1,
        to: Some(0),
    };
    let bytes = encode_enveloped(&envelope, &Message::SeatPart(worst)).expect("encodes");
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
}

#[test]
fn a_seat_is_sent_in_79_parts() {
    assert_eq!(parts_for(SEAT_SIZE_BYTES), 79);
}

#[test]
fn every_receipt_round_trips() {
    for outcome in [
        SeatOutcome::ReceivedWholeAndValid,
        SeatOutcome::Refused(SeatRefusal::WrongSize),
        SeatOutcome::Refused(SeatRefusal::OutOfOrder),
        SeatOutcome::Refused(SeatRefusal::BadChecksum),
        SeatOutcome::Refused(SeatRefusal::NotASave),
    ] {
        let message = Message::SeatReceipt(SeatReceipt {
            transfer: 9,
            outcome,
        });
        let bytes = encode(&message).expect("encodes");
        assert_eq!(decode(&bytes), Ok(message));
    }
}

#[test]
fn a_seat_part_that_cannot_belong_to_its_transfer_is_refused() {
    let mut wrong_count = part(1, 0, SEAT_PART_BYTES);
    wrong_count.parts = 3;
    let mut past_the_end = part(1, 79, SEAT_PART_BYTES);
    past_the_end.part = 79;
    let too_long = part(1, 0, SEAT_PART_BYTES + 1);
    let empty = part(1, 0, 0);
    let mut no_total = part(1, 0, SEAT_PART_BYTES);
    no_total.total_bytes = 0;
    for bad in [wrong_count, past_the_end, too_long, empty, no_total] {
        assert!(encode(&Message::SeatPart(bad)).is_err());
    }
}

#[test]
fn a_card_without_an_id_or_with_a_bad_summary_is_refused() {
    let mut no_id = card(None);
    no_id.campaign_id.clear();
    let mut negative_level = card(None);
    negative_level.host.level = -1;
    let mut not_a_time = card(None);
    not_a_time.host.play_seconds = f64::NAN;
    let mut long_name = card(None);
    long_name.host.name = "x".repeat(MAX_CARD_NAME_BYTES + 1);
    for bad in [no_id, negative_level, not_a_time, long_name] {
        assert!(encode(&Message::CampaignCard(bad)).is_err());
    }
}
