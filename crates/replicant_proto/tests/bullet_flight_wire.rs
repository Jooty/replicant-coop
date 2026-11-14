use replicant_proto::{
    BULLET_STYLE_FLOATS, BulletFlight, BulletLook, MAX_BULLET_AXIS_ABS, MAX_BULLET_EFFECT_ID,
    MAX_BULLET_INSTANCE, MAX_MESSAGE_BYTES, MAX_POSITION_ABS, Message, PROTOCOL_VERSION, WireError,
    decode, encode,
};

fn flight() -> BulletFlight {
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    transform[5] = 1.0;
    transform[10] = 1.0;
    transform[3] = 12.0;
    BulletFlight {
        instance: 120,
        transform,
        velocity: [0.0, 0.0, 30.0],
        look: Some(BulletLook {
            effect_ids: [10000, 10001, 0, 0],
            styles: [[0.5; BULLET_STYLE_FLOATS]; 4],
        }),
        ended: false,
    }
}

#[test]
fn a_bullet_flight_round_trips_fits_a_message_and_is_relayed() {
    let message = Message::BulletFlight(flight());
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_bad_bullet_flight_is_refused() {
    let mut numerous = flight();
    numerous.instance = MAX_BULLET_INSTANCE + 1;
    let mut far = flight();
    far.transform[3] = MAX_POSITION_ABS + 1.0;
    let mut huge_axis = flight();
    huge_axis.transform[0] = MAX_BULLET_AXIS_ABS + 1.0;
    let mut fast = flight();
    fast.velocity[1] = f32::NAN;
    let mut unknown_effect = flight();
    unknown_effect.look.as_mut().unwrap().effect_ids[0] = MAX_BULLET_EFFECT_ID + 1;
    let mut loud = flight();
    loud.look.as_mut().unwrap().styles[1][2] = f32::INFINITY;
    for (bad, field) in [
        (numerous, "bullet instance"),
        (far, "bullet transform"),
        (huge_axis, "bullet transform"),
        (fast, "bullet velocity"),
        (unknown_effect, "bullet effect id"),
        (loud, "bullet style"),
    ] {
        assert_eq!(bad.validate(), Err(WireError::OutOfRange { field }));
    }
}
