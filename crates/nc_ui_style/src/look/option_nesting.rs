use crate::sizes::{OPTION_BRANCH_LABEL_LEFT_PX, OPTION_TOP_LABEL_LEFT_PX};

/// Where an option sits in its tree: at the top level, or nested under the option above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionNesting {
    Top,
    Branch,
    LastBranch,
}

impl OptionNesting {
    /// How far the label starts from the line's left edge.
    pub fn label_left_px(self) -> f32 {
        match self {
            Self::Top => OPTION_TOP_LABEL_LEFT_PX,
            Self::Branch | Self::LastBranch => OPTION_BRANCH_LABEL_LEFT_PX,
        }
    }

    pub fn is_nested(self) -> bool {
        self != Self::Top
    }
}
