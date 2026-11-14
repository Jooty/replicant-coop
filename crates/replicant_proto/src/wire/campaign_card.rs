use serde::{Deserialize, Serialize};

use crate::wire::{PartnerBody, WireError};

pub const MAX_CAMPAIGN_ID_BYTES: usize = 64;
pub const MAX_CARD_NAME_BYTES: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveSummary {
    pub name: String,
    pub level: i32,
    pub play_seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecondSeatCard {
    pub summary: SaveSummary,
    pub body: Option<PartnerBody>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampaignCard {
    pub campaign_id: String,
    pub host: SaveSummary,
    pub second_seat: Option<SecondSeatCard>,
}

impl SaveSummary {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.name.len() > MAX_CARD_NAME_BYTES || self.name.chars().any(char::is_control) {
            return Err(WireError::OutOfRange { field: "name" });
        }
        if self.level < 0 {
            return Err(WireError::OutOfRange { field: "level" });
        }
        if !self.play_seconds.is_finite() || self.play_seconds < 0.0 {
            return Err(WireError::OutOfRange {
                field: "play_seconds",
            });
        }
        Ok(())
    }
}

impl CampaignCard {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.campaign_id.is_empty()
            || self.campaign_id.len() > MAX_CAMPAIGN_ID_BYTES
            || self.campaign_id.chars().any(char::is_control)
        {
            return Err(WireError::OutOfRange {
                field: "campaign_id",
            });
        }
        self.host.validate()?;
        match &self.second_seat {
            Some(seat) => seat.summary.validate(),
            None => Ok(()),
        }
    }
}
