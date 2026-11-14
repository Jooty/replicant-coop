use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoryJumpPoint {
    AfterPrologue,
    AtPopolaTalk,
    BeforeNextDay,
    AtShrineEntrance,
    AtShrineTop,
    AtShrineBoss,
    BossFight3,
    JunkHeapFloorB2,
    JunkHeapBossFight5,
    AfterBossFight6,
    BossFight6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryJump {
    pub point: StoryJumpPoint,
}
