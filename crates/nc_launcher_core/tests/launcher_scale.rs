use nc_launcher_core::LauncherScale;

#[test]
fn the_default_scale_is_medium() {
    assert_eq!(LauncherScale::default(), LauncherScale::Medium);
    assert_eq!(LauncherScale::Large.factor(), 1.0);
}

#[test]
fn stepping_past_the_largest_scale_stays_there() {
    let largest = *LauncherScale::ALL.last().unwrap();
    assert_eq!(largest.larger(), largest);
    assert_eq!(LauncherScale::Medium.larger(), LauncherScale::Large);
}

#[test]
fn stepping_past_the_smallest_scale_stays_there() {
    let smallest = LauncherScale::ALL[0];
    assert_eq!(smallest.smaller(), smallest);
    assert_eq!(LauncherScale::Large.smaller(), LauncherScale::Medium);
}
