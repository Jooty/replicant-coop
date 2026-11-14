use replicant_game::fight_sync::{EYE_SET_CALLERS_LISTED, EyeSetTally, eye_set_text};

#[test]
fn callers_are_counted_and_the_most_frequent_listed_first() {
    let mut tally = EyeSetTally::new();
    for _ in 0..3 {
        tally.record(0x34_7243, false);
    }
    for _ in 0..7 {
        tally.record(0x6F_94D5, false);
    }
    tally.record(0x34_162C, false);
    for extra in 0..10 {
        tally.record(0x50_0000 + extra, false);
    }
    let stretch = tally.take();
    assert_eq!(stretch.by_game, 21);
    assert_eq!(stretch.by_mod, 0);
    assert_eq!(stretch.callers.len(), EYE_SET_CALLERS_LISTED);
    assert_eq!(stretch.callers[0], (0x6F_94D5, 7));
    assert_eq!(stretch.callers[1], (0x34_7243, 3));
    assert!(eye_set_text(&stretch).starts_with(
        "eye set by the game 21 by the mod 0; game callers [0x6F94D5 x7, 0x347243 x3, "
    ));
    assert_eq!(tally.take().by_game, 0, "a take starts the count again");
}

#[test]
fn the_mods_own_calls_are_counted_apart() {
    let mut tally = EyeSetTally::new();
    for _ in 0..4 {
        tally.record(0x6F_94D5, true);
    }
    tally.record(0x34_7243, false);
    let stretch = tally.take();
    assert_eq!(stretch.by_mod, 4);
    assert_eq!(stretch.by_game, 1);
    assert_eq!(stretch.callers, vec![(0x34_7243, 1)]);
}
