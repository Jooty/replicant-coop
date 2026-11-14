use crate::palette::{INK, INK_MUTED, INK_UNAVAILABLE};

/// How a control is drawn and whether it reacts: editable in sand ink, read-only in muted ink (a guest seeing
/// the host's choice), or unavailable in grey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlLook {
    Editable,
    ReadOnly,
    Unavailable,
}

impl ControlLook {
    pub fn editable(self) -> bool {
        self == Self::Editable
    }

    /// The ink of an on/off switch: full sand only when it is on and can be changed.
    pub fn switch_ink(self, on: bool) -> [u8; 4] {
        match (self, on) {
            (Self::Unavailable, _) => INK_UNAVAILABLE,
            (Self::ReadOnly, _) | (Self::Editable, false) => INK_MUTED,
            (Self::Editable, true) => INK,
        }
    }

    /// The ink of an amount: a meter's filled cells and the number beside it.
    pub fn amount_ink(self) -> [u8; 4] {
        match self {
            Self::Editable => INK,
            Self::ReadOnly => INK_MUTED,
            Self::Unavailable => INK_UNAVAILABLE,
        }
    }

    /// The ink of a control's label, which stays readable unless the control is unavailable.
    pub fn label_ink(self) -> [u8; 4] {
        if self == Self::Unavailable {
            INK_UNAVAILABLE
        } else {
            INK
        }
    }
}
