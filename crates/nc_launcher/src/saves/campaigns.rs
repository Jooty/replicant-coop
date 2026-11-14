use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use replicant_game::save_files::COOP_GAMEDATA_FILE_NAME;
use serde::{Deserialize, Serialize};

use crate::saves::coop_gamedata_path;

const COOP_CAMPAIGNS_SUFFIX: &str = ".campaigns.toml";

pub type CampaignRecord = BTreeMap<u8, Campaign>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeatSummary {
    pub name: String,
    pub level: i32,
    pub play_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub id: String,
    pub host_name: String,
    pub host_play_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat: Option<SeatSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrived_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat_checksum: Option<u32>,
}

#[derive(Default, Serialize, Deserialize)]
struct CampaignsFile {
    #[serde(default)]
    campaigns: BTreeMap<String, Campaign>,
}

pub fn coop_campaigns_path(save_dir: &Path) -> Option<PathBuf> {
    let coop = coop_gamedata_path(save_dir)?;
    Some(campaigns_path_beside(&coop))
}

pub(super) fn campaigns_path_beside(coop_file: &Path) -> PathBuf {
    coop_file.with_file_name(format!("{COOP_GAMEDATA_FILE_NAME}{COOP_CAMPAIGNS_SUFFIX}"))
}

pub fn parse_campaigns(text: &str) -> CampaignRecord {
    let file: CampaignsFile = toml::from_str(text).unwrap_or_default();
    file.campaigns
        .into_iter()
        .filter_map(|(slot, campaign)| Some((slot.parse().ok()?, campaign)))
        .collect()
}

pub fn render_campaigns(record: &CampaignRecord) -> String {
    let file = CampaignsFile {
        campaigns: record
            .iter()
            .map(|(slot, campaign)| (slot.to_string(), campaign.clone()))
            .collect(),
    };
    toml::to_string(&file).unwrap_or_default()
}

pub fn read_campaigns(path: &Path) -> CampaignRecord {
    std::fs::read_to_string(path)
        .map(|text| parse_campaigns(&text))
        .unwrap_or_default()
}

pub fn write_campaigns(path: &Path, record: &CampaignRecord) -> io::Result<()> {
    let staging = path.with_extension("tmp");
    std::fs::write(&staging, render_campaigns(record))?;
    std::fs::rename(&staging, path)
}

pub fn store_campaign(path: &Path, slot: u8, campaign: Campaign) -> io::Result<()> {
    let mut record = read_campaigns(path);
    record.insert(slot, campaign);
    write_campaigns(path, &record)
}
