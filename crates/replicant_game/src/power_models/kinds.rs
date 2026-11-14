use replicant_proto::PowerModelKind;

pub const POWER_MODEL_KINDS: [PowerModelKind; 4] = [
    PowerModelKind::BlackHand030,
    PowerModelKind::BlackHand020,
    PowerModelKind::BlackHandAlt030,
    PowerModelKind::BlackHandAlt020,
];

pub fn power_model_pack_name(kind: PowerModelKind) -> &'static str {
    match kind {
        PowerModelKind::BlackHand030 => "blackhand030",
        PowerModelKind::BlackHand020 => "blackhand020",
        PowerModelKind::BlackHandAlt030 => "d_blackhand030",
        PowerModelKind::BlackHandAlt020 => "d_blackhand020",
        PowerModelKind::Bomb010 => "BOMB010",
    }
}

/// A carried prop rather than a power: shown as a plain object wearing its pack, never as the
/// class the game keeps under that name (a real bomb would go off when struck).
pub fn power_model_is_prop(kind: PowerModelKind) -> bool {
    matches!(kind, PowerModelKind::Bomb010)
}
