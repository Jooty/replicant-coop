use replicant_proto::StoryJumpPoint;

const AFTER_PROLOGUE_STEPS: [JumpStep; 18] = [
    JumpStep::SetFlag { id: 71 },
    JumpStep::SetFlag { id: 153 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::ClearFlag { id: 149 },
    JumpStep::SetTutorial { id: 2000 },
    JumpStep::SetTutorial { id: 2012 },
    JumpStep::SetTutorial { id: 2026 },
    JumpStep::SetTutorial { id: 2074 },
    JumpStep::SetTutorial { id: 2386 },
    JumpStep::SetTutorial { id: 2387 },
    JumpStep::SetTutorial { id: 2388 },
    JumpStep::SetTutorial { id: 2389 },
    JumpStep::SetTutorial { id: 2719 },
    JumpStep::SetTutorial { id: 2732 },
    JumpStep::SetItem { id: 727, count: 0 },
    JumpStep::SetItem { id: 1000, count: 1 },
    JumpStep::EquipWeapon { id: 1000 },
    JumpStep::SetPlayerLevel { level: 0 },
];

const POPOLA_TALK_EXTRA_STEPS: [JumpStep; 3] = [
    JumpStep::SetFlag { id: 154 },
    JumpStep::SetFlag { id: 1299 },
    JumpStep::SetFlag { id: 1401 },
];

const BEFORE_NEXT_DAY_EXTRA_STEPS: [JumpStep; 21] = [
    JumpStep::SetFlag { id: 154 },
    JumpStep::SetFlag { id: 73 },
    JumpStep::SetFlag { id: 156 },
    JumpStep::SetFlag { id: 160 },
    JumpStep::SetFlag { id: 161 },
    JumpStep::SetFlag { id: 157 },
    JumpStep::SetFlag { id: 1360 },
    JumpStep::SetFlag { id: 158 },
    JumpStep::SetFlag { id: 112 },
    JumpStep::SetFlag { id: 159 },
    JumpStep::SetFlag { id: 162 },
    JumpStep::SetFlag { id: 1443 },
    JumpStep::SetFlag { id: 165 },
    JumpStep::SetFlag { id: 163 },
    JumpStep::SetFlag { id: 164 },
    JumpStep::SetFlag { id: 166 },
    JumpStep::SetFlag { id: 1300 },
    JumpStep::ClearFlag { id: 1299 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetItem { id: 723, count: 1 },
    JumpStep::SetTutorial { id: 2010 },
];

const AT_SHRINE_ENTRANCE_EXTRA_STEPS: [JumpStep; 32] = [
    JumpStep::SetFlag { id: 167 },
    JumpStep::SetFlag { id: 168 },
    JumpStep::SetFlag { id: 169 },
    JumpStep::SetSnowFlag { id: 1052 },
    JumpStep::SetSnowFlag { id: 1069 },
    JumpStep::SetFlag { id: 170 },
    JumpStep::SetFlag { id: 171 },
    JumpStep::SetFlag { id: 172 },
    JumpStep::SetFlag { id: 173 },
    JumpStep::SetFlag { id: 174 },
    JumpStep::SetFlag { id: 175 },
    JumpStep::SetFlag { id: 176 },
    JumpStep::SetFlag { id: 189 },
    JumpStep::SetFlag { id: 190 },
    JumpStep::SetFlag { id: 191 },
    JumpStep::SetFlag { id: 116 },
    JumpStep::SetFlag { id: 77 },
    JumpStep::SetFlag { id: 1303 },
    JumpStep::ClearFlag { id: 1300 },
    JumpStep::ClearFlag { id: 1443 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::ClearFlag { id: 1404 },
    JumpStep::ClearFlag { id: 1405 },
    JumpStep::ClearFlag { id: 1406 },
    JumpStep::ClearFlag { id: 1407 },
    JumpStep::ClearFlag { id: 1408 },
    JumpStep::ClearFlag { id: 1409 },
    JumpStep::ClearFlag { id: 1410 },
    JumpStep::SetGameCounter {
        id: 100,
        value: 255,
    },
    JumpStep::SetGameCounter {
        id: 101,
        value: 255,
    },
];

const AT_SHRINE_TOP_EXTRA_STEPS: [JumpStep; 7] = [
    JumpStep::SetFlag { id: 75 },
    JumpStep::SetFlag { id: 192 },
    JumpStep::SetFlag { id: 138 },
    JumpStep::SetFlag { id: 193 },
    JumpStep::SetFlag { id: 1432 },
    JumpStep::SetFlag { id: 205 },
    JumpStep::SetItem { id: 305, count: 0 },
];

const AT_SHRINE_BOSS_EXTRA_STEPS: [JumpStep; 3] = [
    JumpStep::SetFlag { id: 194 },
    JumpStep::SetSnowFlag { id: 1053 },
    JumpStep::SetFlag { id: 206 },
];

const AT_SHRINE_ARENA_EXTRA_STEPS: [JumpStep; 9] = [
    JumpStep::ClearFlag { id: 1303 },
    JumpStep::SetFlag { id: 1432 },
    JumpStep::SetFlag { id: 1433 },
    JumpStep::SetFlag { id: 1434 },
    JumpStep::SetFlag { id: 127 },
    JumpStep::SetHaveWhiteBook { on: true },
    JumpStep::LearnMagic { id: 3 },
    JumpStep::SetMagicSlot { slot: 1, magic: 3 },
    JumpStep::SetTutorial { id: 2002 },
];

const AERIE_ENTRANCE_EXTRA_STEPS: [JumpStep; 35] = [
    JumpStep::SetFlag { id: 195 },
    JumpStep::SetFlag { id: 196 },
    JumpStep::SetFlag { id: 198 },
    JumpStep::SetFlag { id: 204 },
    JumpStep::SetFlag { id: 130 },
    JumpStep::LearnMagic { id: 6 },
    JumpStep::SetMagicSlot { slot: 0, magic: 6 },
    JumpStep::SetTutorial { id: 2005 },
    JumpStep::SetTutorial { id: 2020 },
    JumpStep::SetTutorial { id: 2079 },
    JumpStep::SetFlag { id: 200 },
    JumpStep::SetFlag { id: 201 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 202 },
    JumpStep::SetGameCounter { id: 100, value: 3 },
    JumpStep::SetGameCounter { id: 101, value: 3 },
    JumpStep::SetFlag { id: 217 },
    JumpStep::SetFlag { id: 219 },
    JumpStep::SetFlag { id: 1304 },
    JumpStep::SetFlag { id: 1361 },
    JumpStep::SetFlag { id: 218 },
    JumpStep::SetSnowFlag { id: 1085 },
    JumpStep::ClearFlag { id: 1304 },
    JumpStep::SetFlag { id: 220 },
    JumpStep::ClearFlag { id: 1305 },
    JumpStep::SetFlag { id: 221 },
    JumpStep::SetFlag { id: 222 },
    JumpStep::SetFlag { id: 224 },
    JumpStep::SetTutorial { id: 2014 },
    JumpStep::SetFlag { id: 225 },
    JumpStep::SetGameCounter {
        id: 100,
        value: 255,
    },
    JumpStep::SetGameCounter {
        id: 101,
        value: 255,
    },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 239 },
];

const BOSS_FIGHT_3_LEAD_IN_EXTRA_STEPS: [JumpStep; 26] = [
    JumpStep::SetFlag { id: 70 },
    JumpStep::ClearFlag { id: 1433 },
    JumpStep::ClearFlag { id: 1434 },
    JumpStep::SetFlag { id: 79 },
    JumpStep::SetFlag { id: 1442 },
    JumpStep::SetFlag { id: 1401 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::SetFlag { id: 240 },
    JumpStep::SetFlag { id: 241 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1409 },
    JumpStep::ClearFlag { id: 1409 },
    JumpStep::SetFlag { id: 1403 },
    JumpStep::SetFlag { id: 255 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 1407 },
    JumpStep::ClearFlag { id: 1407 },
    JumpStep::SetFlag { id: 1408 },
    JumpStep::SetFlag { id: 242 },
    JumpStep::SetFlag { id: 1451 },
    JumpStep::ClearFlag { id: 1408 },
    JumpStep::SetFlag { id: 1409 },
    JumpStep::ClearFlag { id: 1409 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::SetFlag { id: 243 },
];

const BOSS_FIGHT_3_TO_JUNK_HEAP_FIRST_UPGRADE_EXTRA_STEPS: [JumpStep; 109] = [
    JumpStep::SetFlag { id: 244 },
    JumpStep::SetFlag { id: 245 },
    JumpStep::SetFlag { id: 246 },
    JumpStep::SetFlag { id: 247 },
    JumpStep::SetFlag { id: 248 },
    JumpStep::SetFlag { id: 129 },
    JumpStep::LearnMagic { id: 5 },
    JumpStep::SetTutorial { id: 2004 },
    JumpStep::ClearFlag { id: 1442 },
    JumpStep::SetFlag { id: 137 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1401 },
    JumpStep::ClearFlag { id: 137 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetGameCounter { id: 100, value: 3 },
    JumpStep::SetGameCounter { id: 101, value: 3 },
    JumpStep::SetFlag { id: 249 },
    JumpStep::SetFlag { id: 252 },
    JumpStep::SetFlag { id: 253 },
    JumpStep::SetFlag { id: 1362 },
    JumpStep::SetFlag { id: 1357 },
    JumpStep::ClearFlag { id: 252 },
    JumpStep::SetFlag { id: 323 },
    JumpStep::SetFlag { id: 1355 },
    JumpStep::SetFlag { id: 1356 },
    JumpStep::SetFlag { id: 324 },
    JumpStep::SetFlag { id: 325 },
    JumpStep::ClearFlag { id: 1432 },
    JumpStep::ClearFlag { id: 1451 },
    JumpStep::SetFlag { id: 114 },
    JumpStep::SetFlag { id: 327 },
    JumpStep::SetFlag { id: 256 },
    JumpStep::SetTutorial { id: 2055 },
    JumpStep::SetItem { id: 1009, count: 1 },
    JumpStep::SetFlag { id: 328 },
    JumpStep::SetGameCounter {
        id: 100,
        value: 255,
    },
    JumpStep::SetGameCounter {
        id: 101,
        value: 255,
    },
    JumpStep::SetFlag { id: 81 },
    JumpStep::SetTutorial { id: 2021 },
    JumpStep::SetFlag { id: 337 },
    JumpStep::SetFlag { id: 1432 },
    JumpStep::ClearFlag { id: 1432 },
    JumpStep::SetFlag { id: 338 },
    JumpStep::SetFlag { id: 1316 },
    JumpStep::SetItem { id: 515, count: 1 },
    JumpStep::SetTutorial { id: 2022 },
    JumpStep::SetItem { id: 531, count: 1 },
    JumpStep::SetFlag { id: 339 },
    JumpStep::SetFlag { id: 1317 },
    JumpStep::SetFlag { id: 340 },
    JumpStep::ClearFlag { id: 1317 },
    JumpStep::SetFlag { id: 341 },
    JumpStep::SetFlag { id: 346 },
    JumpStep::SetGameCounter { id: 100, value: 2 },
    JumpStep::SetGameCounter { id: 101, value: 0 },
    JumpStep::ClearFlag { id: 1355 },
    JumpStep::ClearFlag { id: 1356 },
    JumpStep::ClearFlag { id: 1357 },
    JumpStep::SetFlag { id: 1363 },
    JumpStep::SetFlag { id: 358 },
    JumpStep::SetFlag { id: 360 },
    JumpStep::SetFlag { id: 361 },
    JumpStep::SetFlag { id: 268 },
    JumpStep::SetFlag { id: 269 },
    JumpStep::SetFlag { id: 365 },
    JumpStep::SetFlag { id: 271 },
    JumpStep::SetFlag { id: 272 },
    JumpStep::SetFlag { id: 277 },
    JumpStep::SetFlag { id: 275 },
    JumpStep::SetFlag { id: 276 },
    JumpStep::SetFlag { id: 280 },
    JumpStep::SetFlag { id: 367 },
    JumpStep::SetFlag { id: 281 },
    JumpStep::SetFlag { id: 282 },
    JumpStep::SetGameCounter {
        id: 100,
        value: 255,
    },
    JumpStep::SetGameCounter {
        id: 101,
        value: 255,
    },
    JumpStep::SetFlag { id: 98 },
    JumpStep::SetFlag { id: 284 },
    JumpStep::SetFlag { id: 285 },
    JumpStep::SetItem { id: 715, count: 1 },
    JumpStep::SetItem { id: 716, count: 1 },
    JumpStep::SetFlag { id: 283 },
    JumpStep::SetFlag { id: 100 },
    JumpStep::SetFlag { id: 286 },
    JumpStep::SetFlag { id: 1401 },
    JumpStep::SetFlag { id: 291 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1403 },
    JumpStep::SetFlag { id: 290 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 1405 },
    JumpStep::SetFlag { id: 287 },
    JumpStep::SetFlag { id: 292 },
    JumpStep::SetSnowFlag { id: 1051 },
    JumpStep::ClearFlag { id: 1405 },
    JumpStep::SetFlag { id: 1403 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1401 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetFlag { id: 288 },
    JumpStep::SetFlag { id: 301 },
    JumpStep::SetFlag { id: 1314 },
    JumpStep::SetItem { id: 1109, count: 1 },
    JumpStep::EquipWeapon { id: 1109 },
    JumpStep::SetItem { id: 250, count: 1 },
];

const JUNK_HEAP_LIFT_TO_FLOOR_B2_EXTRA_STEPS: [JumpStep; 21] = [
    JumpStep::SetSnowFlag { id: 5 },
    JumpStep::ClearFlag { id: 1314 },
    JumpStep::SetFlag { id: 302 },
    JumpStep::SetFlag { id: 1401 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1406 },
    JumpStep::ClearFlag { id: 1406 },
    JumpStep::SetFlag { id: 1403 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 1404 },
    JumpStep::ClearFlag { id: 1404 },
    JumpStep::SetFlag { id: 1405 },
    JumpStep::ClearFlag { id: 1405 },
    JumpStep::SetFlag { id: 1406 },
    JumpStep::ClearFlag { id: 1406 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1407 },
    JumpStep::ClearFlag { id: 1407 },
];

const JUNK_HEAP_FLOOR_B2_TO_BOSS_FIGHT_5_EXTRA_STEPS: [JumpStep; 18] = [
    JumpStep::SetFlag { id: 1401 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::SetFlag { id: 315 },
    JumpStep::SetTutorial { id: 2052 },
    JumpStep::SetFlag { id: 314 },
    JumpStep::SetTutorial { id: 2050 },
    JumpStep::SetFlag { id: 304 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1403 },
    JumpStep::SetFlag { id: 313 },
    JumpStep::SetFlag { id: 303 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 1404 },
    JumpStep::SetFlag { id: 305 },
    JumpStep::ClearFlag { id: 1404 },
    JumpStep::SetFlag { id: 1405 },
    JumpStep::ClearFlag { id: 1405 },
];

const BOSS_FIGHT_6_LEAD_IN_EXTRA_STEPS: [JumpStep; 16] = [
    JumpStep::SetFlag { id: 102 },
    JumpStep::SetFlag { id: 306 },
    JumpStep::SetFlag { id: 307 },
    JumpStep::SetTutorial { id: 2007 },
    JumpStep::SetFlag { id: 132 },
    JumpStep::LearnMagic { id: 8 },
    JumpStep::SetFlag { id: 308 },
    JumpStep::SetFlag { id: 309 },
    JumpStep::SetFlag { id: 310 },
    JumpStep::SetFlag { id: 374 },
    JumpStep::SetGameCounter { id: 100, value: 2 },
    JumpStep::SetGameCounter { id: 101, value: 1 },
    JumpStep::SetFlag { id: 311 },
    JumpStep::SetSnowFlag { id: 1056 },
    JumpStep::SetGameCounter {
        id: 100,
        value: 255,
    },
    JumpStep::SetGameCounter {
        id: 101,
        value: 255,
    },
];

const BOSS_FIGHT_6_TO_AFTER_EXTRA_STEPS: [JumpStep; 50] = [
    JumpStep::SetFlag { id: 1401 },
    JumpStep::ClearFlag { id: 1401 },
    JumpStep::SetFlag { id: 1402 },
    JumpStep::SetFlag { id: 375 },
    JumpStep::SetFlag { id: 376 },
    JumpStep::ClearFlag { id: 1402 },
    JumpStep::SetFlag { id: 1409 },
    JumpStep::SetFlag { id: 377 },
    JumpStep::SetFlag { id: 378 },
    JumpStep::SetFlag { id: 1307 },
    JumpStep::ClearFlag { id: 1409 },
    JumpStep::SetFlag { id: 1403 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 1404 },
    JumpStep::ClearFlag { id: 1404 },
    JumpStep::SetFlag { id: 1410 },
    JumpStep::ClearFlag { id: 1307 },
    JumpStep::SetFlag { id: 379 },
    JumpStep::ClearFlag { id: 1410 },
    JumpStep::SetFlag { id: 1405 },
    JumpStep::SetFlag { id: 380 },
    JumpStep::SetFlag { id: 381 },
    JumpStep::SetFlag { id: 1308 },
    JumpStep::ClearFlag { id: 1308 },
    JumpStep::SetFlag { id: 382 },
    JumpStep::SetFlag { id: 384 },
    JumpStep::SetFlag { id: 1309 },
    JumpStep::ClearFlag { id: 1405 },
    JumpStep::SetFlag { id: 1406 },
    JumpStep::ClearFlag { id: 1406 },
    JumpStep::SetFlag { id: 1411 },
    JumpStep::ClearFlag { id: 1309 },
    JumpStep::SetFlag { id: 383 },
    JumpStep::SetFlag { id: 385 },
    JumpStep::SetFlag { id: 386 },
    JumpStep::SetFlag { id: 1310 },
    JumpStep::ClearFlag { id: 1411 },
    JumpStep::SetFlag { id: 1410 },
    JumpStep::ClearFlag { id: 1410 },
    JumpStep::SetFlag { id: 1404 },
    JumpStep::ClearFlag { id: 1404 },
    JumpStep::SetFlag { id: 1403 },
    JumpStep::ClearFlag { id: 1310 },
    JumpStep::SetFlag { id: 125 },
    JumpStep::SetFlag { id: 128 },
    JumpStep::LearnMagic { id: 4 },
    JumpStep::SetTutorial { id: 2003 },
    JumpStep::SetFlag { id: 387 },
    JumpStep::ClearFlag { id: 1403 },
    JumpStep::SetFlag { id: 1409 },
];

const AFTER_PROLOGUE_MP_STEP: JumpStep = JumpStep::MpFullMode { on: false };
const AFTER_PROLOGUE_PHASE: &str = "A_CENTER_VILLAGE_01";
const AFTER_PROLOGUE_POINT: i32 = 8;
const POPOLA_TALK_PHASE: &str = "A_CENTER_LIBRARY_01";
const POPOLA_TALK_POINT: i32 = 1;
const BEFORE_NEXT_DAY_PHASE: &str = "A_CENTER_VILLAGE_01";
const BEFORE_NEXT_DAY_POINT: i32 = 4;
const AT_SHRINE_ENTRANCE_PHASE: &str = "A_STONE_TEMPLE_01";
const AT_SHRINE_ENTRANCE_POINT: i32 = 1;
const AT_SHRINE_TOP_PHASE: &str = "A_STONE_FIELD_01";
const AT_SHRINE_TOP_POINT: i32 = 3;
const AT_SHRINE_BOSS_PHASE: &str = "A_STONE_FIELD_02";
const AT_SHRINE_BOSS_POINT: i32 = 1;
const BOSS_FIGHT_3_PHASE: &str = "A_CLIFF_VILLAGE_01";
const BOSS_FIGHT_3_POINT: i32 = 1;
const JUNK_HEAP_FLOOR_B2_PHASE: &str = "A_MOUNT_ROBOT_02";
const JUNK_HEAP_FLOOR_B2_POINT: i32 = 1;
const JUNK_HEAP_BOSS_FIGHT_5_PHASE: &str = "A_MOUNT_ROBOT_10";
const JUNK_HEAP_BOSS_FIGHT_5_POINT: i32 = 1;
const BOSS_FIGHT_6_PHASE: &str = "A_CLIFF_VILLAGE_01";
const BOSS_FIGHT_6_POINT: i32 = 1;
const AFTER_BOSS_FIGHT_6_PHASE: &str = "A_CLIFF_VILLAGE_01";
const AFTER_BOSS_FIGHT_6_POINT: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpStep {
    SetFlag { id: u16 },
    ClearFlag { id: u16 },
    SetTutorial { id: u16 },
    SetSnowFlag { id: u16 },
    SetGameCounter { id: u8, value: u8 },
    SetItem { id: i32, count: i32 },
    EquipWeapon { id: i32 },
    SetPlayerLevel { level: i32 },
    MpFullMode { on: bool },
    SetHaveWhiteBook { on: bool },
    LearnMagic { id: i32 },
    SetMagicSlot { slot: i32, magic: i32 },
}

pub fn loadout_steps(point: StoryJumpPoint) -> Vec<JumpStep> {
    match point {
        StoryJumpPoint::AfterPrologue => {
            let mut steps = AFTER_PROLOGUE_STEPS.to_vec();
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::AtPopolaTalk => {
            let mut steps = AFTER_PROLOGUE_STEPS.to_vec();
            steps.extend_from_slice(&POPOLA_TALK_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::BeforeNextDay => {
            let mut steps = AFTER_PROLOGUE_STEPS.to_vec();
            steps.extend_from_slice(&BEFORE_NEXT_DAY_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::AtShrineEntrance => {
            let mut steps = AFTER_PROLOGUE_STEPS.to_vec();
            steps.extend_from_slice(&BEFORE_NEXT_DAY_EXTRA_STEPS);
            steps.extend_from_slice(&AT_SHRINE_ENTRANCE_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::AtShrineTop => {
            let mut steps = AFTER_PROLOGUE_STEPS.to_vec();
            steps.extend_from_slice(&BEFORE_NEXT_DAY_EXTRA_STEPS);
            steps.extend_from_slice(&AT_SHRINE_ENTRANCE_EXTRA_STEPS);
            steps.extend_from_slice(&AT_SHRINE_TOP_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::AtShrineBoss => {
            let mut steps = AFTER_PROLOGUE_STEPS.to_vec();
            steps.extend_from_slice(&BEFORE_NEXT_DAY_EXTRA_STEPS);
            steps.extend_from_slice(&AT_SHRINE_ENTRANCE_EXTRA_STEPS);
            steps.extend_from_slice(&AT_SHRINE_TOP_EXTRA_STEPS);
            steps.extend_from_slice(&AT_SHRINE_BOSS_EXTRA_STEPS);
            steps.extend_from_slice(&AT_SHRINE_ARENA_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::BossFight3 => {
            let mut steps = loadout_steps(StoryJumpPoint::AtShrineBoss);
            steps.pop();
            steps.extend_from_slice(&AERIE_ENTRANCE_EXTRA_STEPS);
            steps.extend_from_slice(&BOSS_FIGHT_3_LEAD_IN_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::JunkHeapFloorB2 => {
            let mut steps = loadout_steps(StoryJumpPoint::BossFight3);
            steps.pop();
            steps.extend_from_slice(&BOSS_FIGHT_3_TO_JUNK_HEAP_FIRST_UPGRADE_EXTRA_STEPS);
            steps.extend_from_slice(&JUNK_HEAP_LIFT_TO_FLOOR_B2_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::JunkHeapBossFight5 => {
            let mut steps = loadout_steps(StoryJumpPoint::JunkHeapFloorB2);
            steps.pop();
            steps.extend_from_slice(&JUNK_HEAP_FLOOR_B2_TO_BOSS_FIGHT_5_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::BossFight6 => {
            let mut steps = loadout_steps(StoryJumpPoint::JunkHeapBossFight5);
            steps.pop();
            steps.extend_from_slice(&BOSS_FIGHT_6_LEAD_IN_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
        StoryJumpPoint::AfterBossFight6 => {
            let mut steps = loadout_steps(StoryJumpPoint::BossFight6);
            steps.pop();
            steps.extend_from_slice(&BOSS_FIGHT_6_TO_AFTER_EXTRA_STEPS);
            steps.push(AFTER_PROLOGUE_MP_STEP);
            steps
        }
    }
}

pub fn destination(point: StoryJumpPoint) -> (&'static str, i32) {
    match point {
        StoryJumpPoint::AfterPrologue => (AFTER_PROLOGUE_PHASE, AFTER_PROLOGUE_POINT),
        StoryJumpPoint::AtPopolaTalk => (POPOLA_TALK_PHASE, POPOLA_TALK_POINT),
        StoryJumpPoint::BeforeNextDay => (BEFORE_NEXT_DAY_PHASE, BEFORE_NEXT_DAY_POINT),
        StoryJumpPoint::AtShrineEntrance => (AT_SHRINE_ENTRANCE_PHASE, AT_SHRINE_ENTRANCE_POINT),
        StoryJumpPoint::AtShrineTop => (AT_SHRINE_TOP_PHASE, AT_SHRINE_TOP_POINT),
        StoryJumpPoint::AtShrineBoss => (AT_SHRINE_BOSS_PHASE, AT_SHRINE_BOSS_POINT),
        StoryJumpPoint::BossFight3 => (BOSS_FIGHT_3_PHASE, BOSS_FIGHT_3_POINT),
        StoryJumpPoint::JunkHeapFloorB2 => (JUNK_HEAP_FLOOR_B2_PHASE, JUNK_HEAP_FLOOR_B2_POINT),
        StoryJumpPoint::JunkHeapBossFight5 => {
            (JUNK_HEAP_BOSS_FIGHT_5_PHASE, JUNK_HEAP_BOSS_FIGHT_5_POINT)
        }
        StoryJumpPoint::AfterBossFight6 => (AFTER_BOSS_FIGHT_6_PHASE, AFTER_BOSS_FIGHT_6_POINT),
        StoryJumpPoint::BossFight6 => (BOSS_FIGHT_6_PHASE, BOSS_FIGHT_6_POINT),
    }
}
