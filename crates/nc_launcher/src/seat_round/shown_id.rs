use crate::seat_round::CAMPAIGN_ID_SHOWN_CHARACTERS;

pub fn shown_campaign_id(campaign_id: &str) -> &str {
    campaign_id
        .get(..CAMPAIGN_ID_SHOWN_CHARACTERS)
        .unwrap_or(campaign_id)
}
