use replicant_proto::{
    BULLET_STYLE_FLOATS, BulletFlight, BulletLook, BulletSprite, ENEMY_GROUP_COUNT,
    EnemyBulletFlight, EnemyId, MAX_BULLET_INSTANCE, MAX_BULLET_SPRITE_ALPHA,
    MAX_BULLET_SPRITE_ANGLE_ABS, MAX_BULLET_SPRITE_KIND, MAX_BULLET_SPRITE_SIZE, MAX_MESSAGE_BYTES,
    MAX_SHOT_SCALE_GROWTH_PER_SECOND, Message, PROTOCOL_VERSION, WireError, decode, encode,
};

fn enemy_flight() -> EnemyBulletFlight {
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    transform[5] = 1.0;
    transform[10] = 1.0;
    transform[3] = -9.0;
    EnemyBulletFlight {
        enemy: EnemyId {
            group: 3,
            ordinal: 1,
        },
        flight: BulletFlight {
            instance: 300,
            transform,
            velocity: [4.0, 0.0, -2.0],
            look: Some(BulletLook {
                effect_ids: [0x1F7, 0, 0, 0],
                styles: [[0.5; BULLET_STYLE_FLOATS]; 4],
            }),
            ended: false,
        },
        sprite: None,
        scale_growth_per_second: 0.0,
    }
}

fn sprite_flight() -> EnemyBulletFlight {
    let mut flight = enemy_flight();
    flight.flight.look = None;
    flight.sprite = Some(BulletSprite {
        kind: 1,
        size: 0.75,
        alpha: 1.0,
        angle: 1.5,
    });
    flight
}

#[test]
fn an_enemy_bullet_flight_round_trips_fits_a_message_and_is_relayed() {
    let message = Message::EnemyBulletFlight(enemy_flight());
    assert!(message.is_relayed());
    assert_eq!(message.kind_name(), "EnemyBulletFlight");
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn an_enemy_bullet_flight_with_a_bad_enemy_or_flight_is_refused() {
    let mut bad_group = enemy_flight();
    bad_group.enemy.group = ENEMY_GROUP_COUNT;
    assert_eq!(
        bad_group.validate(),
        Err(WireError::OutOfRange {
            field: "enemy_group"
        })
    );
    let mut bad_instance = enemy_flight();
    bad_instance.flight.instance = MAX_BULLET_INSTANCE + 1;
    assert_eq!(
        bad_instance.validate(),
        Err(WireError::OutOfRange {
            field: "bullet instance"
        })
    );
}

#[test]
fn a_sprite_bullet_flight_round_trips_and_fits_a_message() {
    let message = Message::EnemyBulletFlight(sprite_flight());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
}

#[test]
fn a_sprite_the_batch_could_not_draw_is_refused() {
    let sprite = sprite_flight().sprite.unwrap();
    let refused = |sprite: BulletSprite, field: &'static str| {
        let mut flight = sprite_flight();
        flight.sprite = Some(sprite);
        assert_eq!(flight.validate(), Err(WireError::OutOfRange { field }));
    };
    refused(
        BulletSprite {
            kind: MAX_BULLET_SPRITE_KIND + 1,
            ..sprite
        },
        "bullet sprite kind",
    );
    refused(
        BulletSprite {
            size: MAX_BULLET_SPRITE_SIZE * 2.0,
            ..sprite
        },
        "bullet sprite size",
    );
    refused(
        BulletSprite {
            size: f32::NAN,
            ..sprite
        },
        "bullet sprite size",
    );
    refused(
        BulletSprite {
            alpha: -0.5,
            ..sprite
        },
        "bullet sprite alpha",
    );
    refused(
        BulletSprite {
            alpha: MAX_BULLET_SPRITE_ALPHA * 2.0,
            ..sprite
        },
        "bullet sprite alpha",
    );
    refused(
        BulletSprite {
            angle: MAX_BULLET_SPRITE_ANGLE_ABS * 2.0,
            ..sprite
        },
        "bullet sprite angle",
    );
    refused(
        BulletSprite {
            angle: f32::INFINITY,
            ..sprite
        },
        "bullet sprite angle",
    );
}

#[test]
fn a_widening_shot_carries_its_growth_and_a_bad_growth_is_refused() {
    let mut widening = enemy_flight();
    widening.scale_growth_per_second = 17.1;
    let message = Message::EnemyBulletFlight(widening);
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    for bad in [-0.5, f32::NAN, MAX_SHOT_SCALE_GROWTH_PER_SECOND * 2.0] {
        let mut flight = enemy_flight();
        flight.scale_growth_per_second = bad;
        assert_eq!(
            flight.validate(),
            Err(WireError::OutOfRange {
                field: "shot scale growth"
            })
        );
    }
}
