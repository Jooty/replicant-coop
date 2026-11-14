use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::saves::{
    GAMEDATA_FILE_NAME, GAMEDATA_SIZE, SLOT_COUNT, SLOT_LEVEL_OFFSET, SLOT_NAME_LENGTH,
    SLOT_NAME_OFFSET, SLOT_PHASE_LENGTH, SLOT_PHASE_OFFSET, SLOT_PLAYTIME_OFFSET, SLOT_STRIDE,
    SLOT_VERSION_OFFSET, SLOT_VERSIONS, SLOTS_START, STEAM_SAVE_FOLDER,
};

#[derive(Debug, Error)]
pub enum SlotError {
    #[error("GAMEDATA has {0} bytes, expected {GAMEDATA_SIZE}")]
    WrongSize(usize),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotSummary {
    pub index: u8,
    pub used: bool,
    pub name: String,
    pub phase: String,
    pub level: i32,
    pub play_seconds: f64,
}

impl SlotSummary {
    pub fn play_time_text(&self) -> String {
        play_time_text(self.play_seconds)
    }

    /// Hours and minutes the way a save list reads them: "22 h 10".
    pub fn play_time_short(&self) -> String {
        play_time_short(self.play_seconds)
    }
}

pub fn gamedata_path(save_dir: &Path) -> Option<PathBuf> {
    let steam = save_dir.join(STEAM_SAVE_FOLDER);
    let mut found: Vec<PathBuf> = std::fs::read_dir(steam)
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.path().join(GAMEDATA_FILE_NAME)))
        .filter(|path| path.is_file())
        .collect();
    found.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    });
    found.pop()
}

pub fn read_slots(save_dir: &Path) -> Result<Vec<SlotSummary>, SlotError> {
    let path = gamedata_path(save_dir).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no GAMEDATA save file found")
    })?;
    parse_slots(&std::fs::read(path)?)
}

pub fn parse_slots(bytes: &[u8]) -> Result<Vec<SlotSummary>, SlotError> {
    if bytes.len() != GAMEDATA_SIZE {
        return Err(SlotError::WrongSize(bytes.len()));
    }
    Ok((0..SLOT_COUNT)
        .map(|index| {
            let start = SLOTS_START + usize::from(index) * SLOT_STRIDE;
            summarize_slot(index, &bytes[start..start + SLOT_STRIDE])
        })
        .collect())
}

pub fn default_slot(slots: &[SlotSummary]) -> Option<u8> {
    slots
        .iter()
        .filter(|slot| slot.used)
        .max_by(|a, b| a.play_seconds.total_cmp(&b.play_seconds))
        .map(|slot| slot.index)
}

pub fn slot_player_name(slots: &[SlotSummary], selected: Option<u8>) -> Option<String> {
    let slot = slots
        .iter()
        .find(|slot| Some(slot.index) == selected && slot.used)?;
    let name = slot.name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

pub fn play_time_text(play_seconds: f64) -> String {
    let total = play_seconds.max(0.0) as u64;
    format!("{}:{:02}:{:02}", total / 3600, total / 60 % 60, total % 60)
}

pub fn play_time_short(play_seconds: f64) -> String {
    let total = play_seconds.max(0.0) as u64;
    format!("{} h {:02}", total / 3600, total / 60 % 60)
}

pub fn slot_label(index: u8) -> String {
    format!("{:02}", u16::from(index) + 1)
}

pub fn summarize_seat(seat: &[u8]) -> Option<SlotSummary> {
    (seat.len() == SLOT_STRIDE).then(|| summarize_slot(0, seat))
}

fn summarize_slot(index: u8, slot: &[u8]) -> SlotSummary {
    let version = read_u32(slot, SLOT_VERSION_OFFSET);
    let phase = read_text(slot, SLOT_PHASE_OFFSET, SLOT_PHASE_LENGTH);
    SlotSummary {
        index,
        used: SLOT_VERSIONS.contains(&version) && !phase.is_empty(),
        name: read_text(slot, SLOT_NAME_OFFSET, SLOT_NAME_LENGTH),
        phase,
        level: read_u32(slot, SLOT_LEVEL_OFFSET) as i32,
        play_seconds: f64::from_le_bytes(
            slot[SLOT_PLAYTIME_OFFSET..SLOT_PLAYTIME_OFFSET + 8]
                .try_into()
                .unwrap_or_default(),
        ),
    }
}

fn read_u32(slot: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(slot[offset..offset + 4].try_into().unwrap_or_default())
}

fn read_text(slot: &[u8], offset: usize, length: usize) -> String {
    let field = &slot[offset..offset + length];
    let end = field.iter().position(|&byte| byte == 0).unwrap_or(length);
    String::from_utf8_lossy(&field[..end]).into_owned()
}
