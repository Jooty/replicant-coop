mod attack_gauge;
mod block_held;
mod block_moved;
mod bomb_sync;
mod bullet_flight;
mod callback_fired;
mod campaign_card;
mod cart_ride;
mod cart_run;
mod codec;
mod combat_event;
mod container_opened;
mod cutscene_skip;
mod desync_digest;
mod dialogue_line;
mod door_sync;
mod ending_records;
mod enemy;
mod enemy_bullet_flight;
mod enemy_combat;
mod enemy_id_list;
mod fade_sync;
mod fight_camera;
mod fight_stream;
mod follow_phase;
mod game_readiness;
mod gather_for_scene;
mod item_counts;
mod item_pickup;
mod lift_call;
mod lobby;
mod message;
mod message_handler_ran;
mod motion_state;
mod music_state;
mod npc;
mod partner_activity;
mod partner_body;
mod partner_effect;
mod partner_notice;
mod paused_hits;
mod player_settings;
mod player_state;
mod power_model;
mod prologue_level;
mod ridden_boar;
mod scene_follow;
mod script_message;
mod script_roll;
mod seat_transfer;
mod session_options;
mod shared_function_ran;
mod shot_cue;
mod sky_state;
mod story_call;
mod story_jump;
mod story_progress;
mod talk_choice;
mod teleport_here;
mod tutorial_prompt;
mod weapon_state;

pub use attack_gauge::{
    AttackGaugeAsk, AttackGaugeHit, AttackGaugeNode, AttackGaugeOpen, AttackGaugeState,
    MAX_ATTACK_GAUGE_HIT_DAMAGE, MAX_ATTACK_GAUGE_NODE_HP, MAX_ATTACK_GAUGE_NODES,
    MAX_ATTACK_GAUGE_OFFSET_ABS, MAX_ATTACK_GAUGE_SECONDS,
};
pub use block_held::BlockHeld;
pub use block_moved::BlockMoved;
pub use bomb_sync::{BombBlast, WallBroken};
pub use bullet_flight::{
    BULLET_EFFECT_SLOTS, BULLET_STYLE_FLOATS, BULLET_TRANSFORM_FLOATS, BulletFlight, BulletLook,
    MAX_BULLET_AXIS_ABS, MAX_BULLET_EFFECT_ID, MAX_BULLET_INSTANCE, MAX_BULLET_SPEED_ABS,
    MAX_BULLET_STYLE_ABS,
};
pub use callback_fired::CallbackFired;
pub use campaign_card::{
    CampaignCard, MAX_CAMPAIGN_ID_BYTES, MAX_CARD_NAME_BYTES, SaveSummary, SecondSeatCard,
};
pub use cart_ride::CartRide;
pub use cart_run::{CartRideEnd, CartRun, CartRunProgress, MAX_CART_RUN_FRAME, MAX_CART_RUN_RATE};
pub use codec::{decode, encode};
pub use combat_event::CombatEvent;
pub use container_opened::{ContainerFlag, ContainerItem, ContainerOpened};
pub use cutscene_skip::CutsceneSkipAsk;
pub use desync_digest::{
    DesyncDigest, GroupRoster, MAX_DESYNC_GROUP_LINES, MAX_DESYNC_GROUPS,
    PARTNER_POSITION_DESYNC_DISTANCE,
};
pub use dialogue_line::{DialogueLine, DialogueUpdate};
pub use door_sync::{DoorAction, DoorChange, DoorReport, DoorState};
pub use ending_records::{ENDING_ACCOMPLISHED_BYTES, ENDING_COSTUME_BYTES, EndingRecords};
pub use enemy::{
    EnemyCreation, EnemyGone, EnemyGoneReason, EnemyId, EnemySnapshot, EnemySpawned, EnemyState,
};
pub use enemy_bullet_flight::{
    BulletSprite, EnemyBulletFlight, MAX_BULLET_SPRITE_ALPHA, MAX_BULLET_SPRITE_ANGLE_ABS,
    MAX_BULLET_SPRITE_KIND, MAX_BULLET_SPRITE_SIZE, MAX_SHOT_SCALE_GROWTH_PER_SECOND,
};
pub use enemy_combat::{EnemyBodyGone, EnemyDrop, EnemyHit, EnemyKilled, EnemyWordDrop, PlayerHit};
pub use enemy_id_list::EnemyIdList;
pub use fade_sync::FadeSync;
pub use fight_camera::{
    FightCamera, FightCameraAim, FightCameraBodyOffset, FightCameraPose,
    MAX_FIGHT_CAMERA_BLEND_KIND, MAX_FIGHT_CAMERA_BLEND_SECONDS, MAX_FIGHT_CAMERA_INDEX,
};
pub use fight_stream::{
    FightPromptCount, FightRadioWaitEnded, FightRequest, FightRequestAnswer, FightRequestKind,
    FightRequestOutcome, FightRequestRefusal, FightStreamEntry, FightTape, FightTapeValue,
    FightWindowChange, FightWindowMark, MAX_FIGHT_PROMPT_NATIVE_CODE, MAX_FIGHT_PROMPT_PAGES,
    MAX_FIGHT_RADIO_WAIT_INDEX, MAX_FIGHT_TAPE_CALL_INDEX,
};
pub use follow_phase::FollowPhase;
pub use game_readiness::GameReadiness;
pub use gather_for_scene::{GatherAnswer, GatherForScene, GatherReply};
pub use item_counts::{ItemCountEntry, ItemCounts, ItemTake, MAX_ITEM_COUNT_ENTRIES};
pub use item_pickup::{ItemPickup, PickupSource};
pub use lift_call::{LiftCall, LiftCallAnswer, LiftCallOutcome};
pub use lobby::{
    LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, LobbyBody, LobbyReady, LobbyStart, MAX_LOBBY_SLOT,
};
pub use message::{Message, PathInfo};
pub use message_handler_ran::MessageHandlerRan;
pub use motion_state::MotionState;
pub use music_state::{
    MAX_MUSIC_FADE_SECONDS, MAX_MUSIC_POSITION_MS, MAX_MUSIC_VOLUME_ABS, MUSIC_LAYER_COUNT,
    MusicState, NO_MUSIC_TRACK,
};
pub use nc_wire::{BuildGate, Hello, Member, Ping, Pong, Reject, RejectReason, Welcome, WireError};
pub use npc::{NpcId, NpcSnapshot, NpcState, NpcTalk};
pub use partner_activity::PartnerActivity;
pub use partner_body::{
    BodyPackName, GUEST_BODY, HOST_BODY, MAX_BODY_PACK_NAME_BYTES, NO_MOTION_NAME_HASH,
    PartnerBody, motion_name_hash,
};
pub use partner_effect::{
    BulletEffectPlay, EFFECT_ATTACH_BYTES, EFFECT_PARAM_BYTES, EffectLook, EffectPlay, EffectStop,
    MAX_EFFECT_BONE_BYTES, MAX_EFFECT_NAME_BYTES, MAX_SOUND_NAME_BYTES, SoundPlay,
    WORLD_EFFECT_ARGUMENT_COUNT, WORLD_EFFECT_TRANSFORM_FLOATS, WorldEffectMove, WorldEffectPlay,
};
pub use partner_effect::{MAX_VOICE_CHARACTER_BYTES, VoiceCharacter};
pub use partner_notice::PartnerNotice;
pub use paused_hits::PausedHits;
pub use player_settings::PlayerSettings;
pub use player_state::PlayerState;
pub use power_model::{
    MAX_POWER_MODEL_INSTANCE, PowerModelEnded, PowerModelKind, PowerModelMotion, PowerModelState,
};
pub use prologue_level::{
    MAX_PROLOGUE_LEVEL, MAX_REPORTED_WEAPON_ID, MAX_REPORTED_WEAPON_LEVEL, NO_WEAPON_REPORTED,
    PrologueLevel,
};
pub use ridden_boar::{MAX_RIDDEN_BOAR_OFFSET_ABS, RiddenBoar};
pub use scene_follow::{MAX_SCENE_FOLLOW_CALLBACKS, SceneFollow};
pub use script_message::{MAX_SCRIPT_MESSAGE_NUMBER, ScriptMessage};
pub use script_roll::{MAX_SCRIPT_ROLL_ORDINAL, ScriptRoll, ScriptRollValue};
pub use seat_transfer::{
    MAX_SEAT_TRANSFER_BYTES, SEAT_PART_BYTES, SeatOutcome, SeatPart, SeatReceipt, SeatRefusal,
    parts_for,
};
pub use session_options::{
    DEFAULT_ENEMY_DAMAGE_PERCENT, DEFAULT_ENEMY_HEALTH_PERCENT, MAX_ENEMY_DAMAGE_PERCENT,
    MAX_ENEMY_HEALTH_PERCENT, SessionOptions,
};
pub use shared_function_ran::SharedFunctionRan;
pub use shot_cue::{
    MAX_SHOT_CUE_ID, MAX_SHOT_SOUND_LEVEL_ABS, SHOT_SOUND_LEVELS, ShotCue, ShotCueKind,
};
pub use sky_state::{SkyAssets, SkyRecordChunk, SkyState};
pub(crate) use story_call::validate_story_function_and_args;
pub use story_call::{
    MAX_STORY_CALL_ARGS, MAX_STORY_CALL_NAME_BYTES, MAX_STORY_CALL_TEXT_BYTES, StoryCall,
    StoryCallArg, StoryRefusalReason, StoryRefused,
};
pub use story_jump::{StoryJump, StoryJumpPoint};
pub use story_progress::{
    StoryArray, StoryArrayPart, StoryChange, StoryChanges, StoryDigest, StoryRequest, StoryWrite,
    StoryWriteSource,
};
pub use talk_choice::{ChoiceHover, TalkChoice, TalkChoiceKind};
pub use teleport_here::TeleportHere;
pub use tutorial_prompt::{FIRST_TUTORIAL_ID, LAST_TUTORIAL_ID, TutorialPrompt};
pub use weapon_state::WeaponState;

pub const PROTOCOL_VERSION: u16 = 67;
pub const NO_SIDE_VIEW_DOOR: u16 = 0;
pub const MAX_BLOCK_ID: u16 = 1023;
pub const ENEMY_SPEC_BYTES: usize = 160;
pub const ENEMY_SPEC_SLOT_COUNT: u16 = 256;
pub const MAX_ENEMY_ACTOR_NAME_BYTES: usize = 32;
pub const MAX_ENEMY_SCALE: f32 = 100.0;
pub const MAX_FADE_SECONDS: f32 = 30.0;
pub use nc_wire::MAX_MESSAGE_BYTES;
pub const MAX_DISPLAY_NAME_BYTES: usize = 32;
pub const MAX_PLAYERS: u8 = 2;
pub const MAX_PHASE_POINT: i32 = 99;
pub const MAX_HP: u32 = 1_000_000;
pub const MAX_POSITION_ABS: f32 = 1_000_000.0;
pub const MAX_WARP_POINT_ID: u16 = 20_000;
pub const MAX_YAW_ABS_RADIANS: f32 = 16.0;
pub const MAX_MOTION_FRAME: f32 = 100_000.0;
pub const MAX_MOTION_SPEED_ABS: f32 = 32.0;
pub const MAX_WEAPON_ID: u8 = 63;
pub const MAX_WEAPON_LEVEL: u8 = 3;
pub const ENEMY_GROUP_COUNT: u16 = 0x400;
pub const COMPANION_ENEMY_GROUP: u16 = ENEMY_GROUP_COUNT + 1;
pub const COMPANION_SLOT_COUNT: u16 = 3;
pub const MAX_ENEMIES_PER_SNAPSHOT: usize = 5;
pub const MAX_ENEMY_SNAPSHOT_PARTS: u8 = 16;
pub const MAX_ENEMY_IDS_PER_LIST_PART: usize = 64;
pub const MAX_ENEMY_ID_LIST_PARTS: u8 = 8;
pub const NPC_UNGROUPED: u16 = u16::MAX;
pub const MAX_NPCS_PER_SNAPSHOT: usize = 10;
pub const MAX_NPC_SNAPSHOT_PARTS: u8 = 8;
pub const MAX_SKY_AREA: i32 = 127;
pub const MAX_SKY_BLEND_SECONDS: f32 = 120.0;
pub const MAX_ITEM_ID: u16 = 0x2FF;
pub const MAX_ITEM_COUNT: u8 = 99;
pub const MAX_WORD_ID: u16 = 0x7F;
pub const MAX_TALK_ID: u32 = 0x3_FFFF;
pub const MAX_DIALOGUE_LINE_INDEX: u16 = 0x0FFF;
pub const MAX_SKY_RECORD_BYTES: usize = 0xAA0;
pub const MAX_SKY_RECORD_CHUNK_BYTES: usize = 384;
pub const SKY_ASSET_SLOTS: usize = 5;
pub const MAX_HIT_DAMAGE: u32 = 100_000;
pub const MAX_HIT_PUSH_STRENGTH: f32 = 10_000.0;
pub const MAX_SKY_ASSET_NAME_BYTES: usize = 80;
pub const MAX_DOOR_LAYOUT_ID: u16 = 9999;
pub const MAX_DOORS_PER_REPORT: usize = 60;
pub const MIN_RUNNING_CART_STATE: u8 = 5;
pub const MAX_CART_STATE: u8 = 8;
pub const STORY_ARRAY_BYTES: usize = 0x100;
pub const KEY_FRAGMENT_COUNT: usize = 5;
pub const MAX_STORY_FLAG_ID: u16 = 0x7FF;
pub const MAX_STORY_CHANGES_PER_MESSAGE: usize = 80;
pub const MAX_CONTAINER_ITEMS: usize = 8;
pub const MAX_CONTAINER_FLAGS: usize = 16;
pub const MAX_CONTAINER_GOLD: u32 = 9_999_999;
pub const MAX_TALK_CHOICE_INDEX: u8 = 63;
