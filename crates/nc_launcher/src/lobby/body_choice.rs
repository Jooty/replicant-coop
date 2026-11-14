use nc_launcher_core::REPLICANT;
use replicant_proto::PartnerBody;

const NIER_CHARACTER_ID: &str = "nier";
const SWAPPED_CHARACTER_ID: &str = "kaine";

pub fn character_id_for_body(body: PartnerBody) -> Option<&'static str> {
    match body {
        PartnerBody::SeedKaineE => Some(SWAPPED_CHARACTER_ID),
        PartnerBody::Nier
        | PartnerBody::PrologueCharacter
        | PartnerBody::SeedNierY
        | PartnerBody::SeedNierF => Some(NIER_CHARACTER_ID),
    }
}

pub fn body_label(body: PartnerBody) -> &'static str {
    character_id_for_body(body)
        .and_then(|id| REPLICANT.character(id))
        .map_or("", |character| character.label)
}
