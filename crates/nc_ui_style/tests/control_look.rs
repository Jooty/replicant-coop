use nc_ui_style::{
    ControlLook, INK, INK_MUTED, INK_UNAVAILABLE, OPTION_BRANCH_LABEL_LEFT_PX,
    OPTION_TOP_LABEL_LEFT_PX, OptionNesting,
};

#[test]
fn only_an_editable_switch_that_is_on_is_full_ink() {
    assert_eq!(ControlLook::Editable.switch_ink(true), INK);
    assert_eq!(ControlLook::Editable.switch_ink(false), INK_MUTED);
    assert_eq!(ControlLook::ReadOnly.switch_ink(true), INK_MUTED);
}

#[test]
fn an_unavailable_control_is_grey_whatever_its_state() {
    assert_eq!(ControlLook::Unavailable.switch_ink(true), INK_UNAVAILABLE);
    assert_eq!(ControlLook::Unavailable.amount_ink(), INK_UNAVAILABLE);
    assert_eq!(ControlLook::Unavailable.label_ink(), INK_UNAVAILABLE);
}

#[test]
fn a_read_only_label_stays_readable() {
    assert_eq!(ControlLook::ReadOnly.label_ink(), INK);
    assert_eq!(ControlLook::ReadOnly.amount_ink(), INK_MUTED);
    assert!(!ControlLook::ReadOnly.editable());
}

#[test]
fn nested_options_start_further_right() {
    assert_eq!(OptionNesting::Top.label_left_px(), OPTION_TOP_LABEL_LEFT_PX);
    assert_eq!(
        OptionNesting::LastBranch.label_left_px(),
        OPTION_BRANCH_LABEL_LEFT_PX
    );
    assert!(OptionNesting::Branch.is_nested());
    assert!(!OptionNesting::Top.is_nested());
}
