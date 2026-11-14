use crate::enemies::attack_record::ActorHandle;
use crate::memory::GameMemory;

pub const BOSS_BAR_INIT_FUNCTION_RVA: u32 = 0xC_CE70;
pub const BOSS_BAR_SCRIPT_COMMAND_RVA: u32 = 0x6E_E340;
pub const BOSS_BAR_DUMP_INTERVAL_MS: u64 = 5000;
pub const BOSS_BAR_DUMP_BYTES: usize = 64;
const BOSS_BAR_OBJECT_RVA: u32 = 0x277_E230;
const BOSS_BAR_SUB_OBJECT_OFFSET: u64 = 0x288;
const BOSS_BAR_HANDLE_OFFSET: u64 = 0x70;
const BOSS_BAR_STATE_OFFSET: u64 = 0x8;
const BOSS_BAR_STATE_NEVER_OPENED: i32 = 0;
const BOSS_BAR_STATE_CLOSED: i32 = 3;
const UI_MANAGER_RVA: u32 = 0x17B_90D0;
const UI_MANAGER_HUD_OFFSET: u64 = 0x98;
const HUD_BOSS_LIFE_WIDGET_OFFSET: u64 = 0x1D0;
const HUD_BOSS_LIFE_DIGI_WIDGET_OFFSET: u64 = 0x2B8;
const SCRIPT_ARGUMENTS_OFFSET: u64 = 0x18;
const SCRIPT_ARGUMENT_STRIDE: u64 = 0x90;
const SCRIPT_ARGUMENT_TAG_INT: i32 = 4;
const SCRIPT_ARGUMENT_TAG_DOUBLE: i32 = 1;
const SCRIPT_ARGUMENT_VALUE_OFFSET: usize = 8;
const SCRIPT_ARGUMENT_READ_BYTES: usize = 16;

pub type BossBarDump = [u8; BOSS_BAR_DUMP_BYTES];

pub fn script_argument_value(raw: &[u8; SCRIPT_ARGUMENT_READ_BYTES]) -> i32 {
    let tag = i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let value = &raw[SCRIPT_ARGUMENT_VALUE_OFFSET..SCRIPT_ARGUMENT_READ_BYTES];
    if tag == SCRIPT_ARGUMENT_TAG_INT {
        i32::from_le_bytes([value[0], value[1], value[2], value[3]])
    } else if tag == SCRIPT_ARGUMENT_TAG_DOUBLE {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(value);
        f64::from_le_bytes(bytes) as i32
    } else {
        0
    }
}

pub fn read_script_argument<M: GameMemory>(memory: &M, command: u64, index: u64) -> Option<i32> {
    let address = command + SCRIPT_ARGUMENTS_OFFSET + index * SCRIPT_ARGUMENT_STRIDE;
    let raw = memory
        .read_array::<SCRIPT_ARGUMENT_READ_BYTES>(address)
        .ok()?;
    Some(script_argument_value(&raw))
}

pub fn read_boss_bar_handle<M: GameMemory>(memory: &M) -> Option<ActorHandle> {
    ActorHandle::read(memory, boss_bar_sub_object(memory) + BOSS_BAR_HANDLE_OFFSET)
}

/// The boss life gauge (the `CBossLifeGauge` at HUD `+0x288`) is up: its state is neither 0 (never
/// opened) nor 3 (closed by `_CloseBossLifeGauge`), as the gauge's own vslot `0x70` reads it.
pub fn boss_bar_state_is_open(state: i32) -> bool {
    state != BOSS_BAR_STATE_NEVER_OPENED && state != BOSS_BAR_STATE_CLOSED
}

pub fn read_boss_bar_open<M: GameMemory>(memory: &M) -> Option<bool> {
    memory
        .read_i32(boss_bar_sub_object(memory) + BOSS_BAR_STATE_OFFSET)
        .ok()
        .map(boss_bar_state_is_open)
}

pub fn boss_bar_init_text(handle: Option<ActorHandle>, enemy: Option<&str>) -> String {
    let handle = handle.map_or_else(
        || "?".to_owned(),
        |handle| format!("{}/{}", handle.index, handle.generation),
    );
    format!(
        "boss bar: init (handle {handle}, enemy {})",
        enemy.unwrap_or("?")
    )
}

pub fn boss_bar_script_text(first: Option<i32>, second: Option<i32>) -> String {
    let argument = |value: Option<i32>| value.map_or_else(|| "?".to_owned(), |v| v.to_string());
    format!(
        "boss bar: script command (args {}, {})",
        argument(first),
        argument(second)
    )
}

pub fn boss_bar_dump_text(
    life: Option<&BossBarDump>,
    digi: Option<&BossBarDump>,
    bar: Option<&BossBarDump>,
) -> String {
    format!(
        "boss bar: hud life widget +{HUD_BOSS_LIFE_WIDGET_OFFSET:#x} {}, digi +{HUD_BOSS_LIFE_DIGI_WIDGET_OFFSET:#x} {}, bar object {}",
        dump_hex(life),
        dump_hex(digi),
        dump_hex(bar)
    )
}

pub fn read_boss_bar_dump_text<M: GameMemory>(memory: &M) -> String {
    let hud = memory
        .read_u64(memory.module_base() + u64::from(UI_MANAGER_RVA) + UI_MANAGER_HUD_OFFSET)
        .ok()
        .filter(|hud| *hud != 0);
    let widget = |offset: u64| {
        hud.and_then(|hud| memory.read_array::<BOSS_BAR_DUMP_BYTES>(hud + offset).ok())
    };
    let bar = memory
        .read_array::<BOSS_BAR_DUMP_BYTES>(boss_bar_sub_object(memory))
        .ok();
    boss_bar_dump_text(
        widget(HUD_BOSS_LIFE_WIDGET_OFFSET).as_ref(),
        widget(HUD_BOSS_LIFE_DIGI_WIDGET_OFFSET).as_ref(),
        bar.as_ref(),
    )
}

fn boss_bar_sub_object<M: GameMemory>(memory: &M) -> u64 {
    memory.module_base() + u64::from(BOSS_BAR_OBJECT_RVA) + BOSS_BAR_SUB_OBJECT_OFFSET
}

fn dump_hex(dump: Option<&BossBarDump>) -> String {
    dump.map_or_else(
        || "unreadable".to_owned(),
        |bytes| bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
    )
}
