mod slot;
mod track;
mod wire;

pub use slot::PlayerSlot;
pub use track::{
    MAX_MOTION_EXTRAPOLATION_MS, MAX_TRACK_SAMPLES, MOTION_FRAMES_PER_MS, StateTrack,
    TRACK_HISTORY_MS, TRACK_RESTART_GAP_MS, TRACK_RESTART_TICK_JUMP, TrackSample,
};
pub use wire::ChoiceHover;
pub use wire::TeleportHere;
pub use wire::{
    AttackGaugeAsk, AttackGaugeHit, AttackGaugeNode, AttackGaugeOpen, AttackGaugeState,
    MAX_ATTACK_GAUGE_HIT_DAMAGE, MAX_ATTACK_GAUGE_NODE_HP, MAX_ATTACK_GAUGE_NODES,
    MAX_ATTACK_GAUGE_OFFSET_ABS, MAX_ATTACK_GAUGE_SECONDS,
};
pub use wire::{
    BULLET_EFFECT_SLOTS, BULLET_STYLE_FLOATS, BULLET_TRANSFORM_FLOATS, BulletFlight, BulletLook,
    MAX_BULLET_AXIS_ABS, MAX_BULLET_EFFECT_ID, MAX_BULLET_INSTANCE, MAX_BULLET_SPEED_ABS,
    MAX_BULLET_STYLE_ABS,
};
pub use wire::{BlockHeld, BlockMoved, MAX_BLOCK_ID};
pub use wire::{
    BodyPackName, GUEST_BODY, HOST_BODY, MAX_BODY_PACK_NAME_BYTES, NO_MOTION_NAME_HASH,
    PartnerBody, motion_name_hash,
};
pub use wire::{BombBlast, WallBroken};
pub use wire::{
    BuildGate, BulletEffectPlay, COMPANION_ENEMY_GROUP, COMPANION_SLOT_COUNT, CombatEvent,
    CutsceneSkipAsk, DialogueLine, DialogueUpdate, EFFECT_ATTACH_BYTES, EFFECT_PARAM_BYTES,
    ENEMY_GROUP_COUNT, ENEMY_SPEC_BYTES, EffectPlay, EffectStop, EnemyBodyGone, EnemyCreation,
    EnemyDrop, EnemyGone, EnemyGoneReason, EnemyHit, EnemyId, EnemyIdList, EnemyKilled,
    EnemySnapshot, EnemySpawned, EnemyState, EnemyWordDrop, FadeSync, FollowPhase, GameReadiness,
    Hello, MAX_DIALOGUE_LINE_INDEX, MAX_DISPLAY_NAME_BYTES, MAX_EFFECT_BONE_BYTES,
    MAX_EFFECT_NAME_BYTES, MAX_ENEMIES_PER_SNAPSHOT, MAX_ENEMY_ACTOR_NAME_BYTES,
    MAX_ENEMY_ID_LIST_PARTS, MAX_ENEMY_IDS_PER_LIST_PART, MAX_ENEMY_SNAPSHOT_PARTS,
    MAX_FADE_SECONDS, MAX_HIT_DAMAGE, MAX_HIT_PUSH_STRENGTH, MAX_HP, MAX_MESSAGE_BYTES,
    MAX_MOTION_FRAME, MAX_MOTION_SPEED_ABS, MAX_MUSIC_FADE_SECONDS, MAX_MUSIC_POSITION_MS,
    MAX_MUSIC_VOLUME_ABS, MAX_NPC_SNAPSHOT_PARTS, MAX_NPCS_PER_SNAPSHOT, MAX_PHASE_POINT,
    MAX_PLAYERS, MAX_POSITION_ABS, MAX_SKY_AREA, MAX_SKY_ASSET_NAME_BYTES, MAX_SKY_BLEND_SECONDS,
    MAX_SKY_RECORD_BYTES, MAX_SKY_RECORD_CHUNK_BYTES, MAX_SOUND_NAME_BYTES, MAX_TALK_ID,
    MAX_WEAPON_ID, MAX_WEAPON_LEVEL, MAX_YAW_ABS_RADIANS, MUSIC_LAYER_COUNT, Member, Message,
    MotionState, MusicState, NO_MUSIC_TRACK, NPC_UNGROUPED, NpcId, NpcSnapshot, NpcState, NpcTalk,
    PROTOCOL_VERSION, PartnerActivity, PartnerNotice, PathInfo, Ping, PlayerHit, PlayerSettings,
    PlayerState, Pong, Reject, RejectReason, SessionOptions, SkyAssets, SkyRecordChunk, SkyState,
    SoundPlay, WeaponState, Welcome, WireError, decode, encode,
};
pub use wire::{
    BulletSprite, EnemyBulletFlight, MAX_BULLET_SPRITE_ALPHA, MAX_BULLET_SPRITE_ANGLE_ABS,
    MAX_BULLET_SPRITE_KIND, MAX_BULLET_SPRITE_SIZE, MAX_SHOT_SCALE_GROWTH_PER_SECOND,
};
pub use wire::{
    CallbackFired, FightPromptCount, FightRadioWaitEnded, FightRequest, FightRequestAnswer,
    FightRequestKind, FightRequestOutcome, FightRequestRefusal, FightStreamEntry, FightTape,
    FightTapeValue, FightWindowChange, FightWindowMark, MAX_FIGHT_PROMPT_NATIVE_CODE,
    MAX_FIGHT_PROMPT_PAGES, MAX_FIGHT_RADIO_WAIT_INDEX, MAX_FIGHT_TAPE_CALL_INDEX,
    MAX_SCENE_FOLLOW_CALLBACKS, MessageHandlerRan, PausedHits, SceneFollow, SharedFunctionRan,
};
pub use wire::{
    CampaignCard, MAX_CAMPAIGN_ID_BYTES, MAX_CARD_NAME_BYTES, MAX_SEAT_TRANSFER_BYTES,
    SEAT_PART_BYTES, SaveSummary, SeatOutcome, SeatPart, SeatReceipt, SeatRefusal, SecondSeatCard,
    parts_for,
};
pub use wire::{
    CartRide, CartRideEnd, CartRun, CartRunProgress, MAX_CART_RUN_FRAME, MAX_CART_RUN_RATE,
    MAX_CART_STATE, MIN_RUNNING_CART_STATE,
};
pub use wire::{
    ContainerFlag, ContainerItem, ContainerOpened, MAX_CONTAINER_FLAGS, MAX_CONTAINER_GOLD,
    MAX_CONTAINER_ITEMS,
};
pub use wire::{
    DEFAULT_ENEMY_DAMAGE_PERCENT, DEFAULT_ENEMY_HEALTH_PERCENT, MAX_ENEMY_DAMAGE_PERCENT,
    MAX_ENEMY_HEALTH_PERCENT,
};
pub use wire::{
    DesyncDigest, GroupRoster, MAX_DESYNC_GROUP_LINES, MAX_DESYNC_GROUPS,
    PARTNER_POSITION_DESYNC_DISTANCE,
};
pub use wire::{
    DoorAction, DoorChange, DoorReport, DoorState, MAX_DOOR_LAYOUT_ID, MAX_DOORS_PER_REPORT,
};
pub use wire::{ENDING_ACCOMPLISHED_BYTES, ENDING_COSTUME_BYTES, EndingRecords};
pub use wire::{
    EffectLook, WORLD_EFFECT_ARGUMENT_COUNT, WORLD_EFFECT_TRANSFORM_FLOATS, WorldEffectMove,
    WorldEffectPlay,
};
pub use wire::{FIRST_TUTORIAL_ID, LAST_TUTORIAL_ID, TutorialPrompt};
pub use wire::{
    FightCamera, FightCameraAim, FightCameraBodyOffset, FightCameraPose,
    MAX_FIGHT_CAMERA_BLEND_KIND, MAX_FIGHT_CAMERA_BLEND_SECONDS, MAX_FIGHT_CAMERA_INDEX,
};
pub use wire::{GatherAnswer, GatherForScene, GatherReply, NO_SIDE_VIEW_DOOR};
pub use wire::{ItemCountEntry, ItemCounts, ItemTake, MAX_ITEM_COUNT_ENTRIES};
pub use wire::{ItemPickup, MAX_ITEM_COUNT, MAX_ITEM_ID, MAX_WORD_ID, PickupSource};
pub use wire::{
    KEY_FRAGMENT_COUNT, MAX_STORY_CHANGES_PER_MESSAGE, MAX_STORY_FLAG_ID, MAX_TALK_CHOICE_INDEX,
    STORY_ARRAY_BYTES, StoryArray, StoryArrayPart, StoryChange, StoryChanges, StoryDigest,
    StoryRequest, StoryWrite, StoryWriteSource, TalkChoice, TalkChoiceKind,
};
pub use wire::{
    LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, LobbyBody, LobbyReady, LobbyStart, MAX_LOBBY_SLOT,
};
pub use wire::{LiftCall, LiftCallAnswer, LiftCallOutcome, MAX_WARP_POINT_ID};
pub use wire::{
    MAX_POWER_MODEL_INSTANCE, PowerModelEnded, PowerModelKind, PowerModelMotion, PowerModelState,
};
pub use wire::{
    MAX_PROLOGUE_LEVEL, MAX_REPORTED_WEAPON_ID, MAX_REPORTED_WEAPON_LEVEL, NO_WEAPON_REPORTED,
    PrologueLevel,
};
pub use wire::{MAX_RIDDEN_BOAR_OFFSET_ABS, RiddenBoar};
pub use wire::{
    MAX_SCRIPT_MESSAGE_NUMBER, MAX_SCRIPT_ROLL_ORDINAL, ScriptMessage, ScriptRoll, ScriptRollValue,
};
pub use wire::{
    MAX_SHOT_CUE_ID, MAX_SHOT_SOUND_LEVEL_ABS, SHOT_SOUND_LEVELS, ShotCue, ShotCueKind,
};
pub use wire::{
    MAX_STORY_CALL_ARGS, MAX_STORY_CALL_NAME_BYTES, MAX_STORY_CALL_TEXT_BYTES, StoryCall,
    StoryCallArg, StoryRefusalReason, StoryRefused,
};
pub use wire::{MAX_VOICE_CHARACTER_BYTES, VoiceCharacter};
pub use wire::{StoryJump, StoryJumpPoint};
