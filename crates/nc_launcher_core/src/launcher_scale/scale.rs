use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LauncherScale {
    #[default]
    Medium,
    Large,
}

impl LauncherScale {
    pub const ALL: [Self; 2] = [Self::Medium, Self::Large];

    pub fn factor(self) -> f32 {
        match self {
            Self::Medium => 0.8,
            Self::Large => 1.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }

    pub fn larger(self) -> Self {
        let index = self.index();
        Self::ALL[(index + 1).min(Self::ALL.len() - 1)]
    }

    pub fn smaller(self) -> Self {
        Self::ALL[self.index().saturating_sub(1)]
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|scale| *scale == self)
            .unwrap_or_default()
    }
}
