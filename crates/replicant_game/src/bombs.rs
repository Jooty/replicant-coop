mod broken_walls;
mod own_bombs;
mod scan;
mod walls;

pub use broken_walls::{BrokenWalls, same_wall};
pub use own_bombs::{OwnBombChanges, OwnBombHits, OwnBombs};
pub use scan::{
    SightedBomb, SightedBreakable, nearest_breakable, pool_actors, read_bombs, read_breakables,
};
pub use walls::{
    BreakableClass, BreakableState, bomb_break_record, break_is_shared, break_started,
    breakable_class,
};

pub const BREAKABLE_BOX_VTABLE_RVA: u32 = 0x00B0_4230;
pub const SECOND_BREAKABLE_BOX_VTABLE_RVA: u32 = 0x00B0_4B70;
pub const BREAKABLE_RECEIVE_HIT_FUNCTION_RVA: u32 = 0x0016_8790;
pub const BREAKABLE_BOMB_ONLY_MODE_OFFSET: u64 = 0x1_4DE8;
pub const BREAKABLE_BREAK_TIMER_OFFSET: u64 = 0x1_4E00;
pub const BREAKABLE_HITS_LEFT_OFFSET: u64 = 0x1_4E04;
pub const BREAKABLE_BOMB_ONLY_MODE: i32 = 1;
pub const BREAKABLE_HITS_LEFT_FOR_ONE_LAST_HIT: i32 = 1;
pub const BOMB_BLAST_HIT_FLAG: u32 = 0x8000;
pub const BOMB_BLAST_ATTACK_FLAGS: u32 = 0x8080;
pub const ATTACK_DIRECTION_GIVEN_FLAG: u32 = 0x20;
pub const SYNTHETIC_ATTACK_RECORD_BYTES: usize = 0x120;
pub const WALL_MATCH_DISTANCE: f32 = 2.0;
pub const SAME_WALL_DISTANCE: f32 = 0.5;
pub const WALL_RETRY_INTERVAL_MS: u64 = 500;
pub const WALL_GIVE_UP_MS: u64 = 15_000;
pub const MAX_PENDING_WALLS: usize = 16;
pub const BOMB_VTABLE_RVA: u32 = 0x00B0_5DF0;
pub const BOMB_EXPLODE_FUNCTION_RVA: u32 = 0x0016_1B50;
pub const BOMB_HOLDER_HANDLE_OFFSET: u64 = 0x1_4DC4;
pub const ATTACK_HIT_SERIAL_RVA: u32 = 0x011A_F344;
pub const BOMB_BLAST_EFFECT_ID: i32 = 0x1F64;
pub const BOMB_BLAST_SOUND_ID: u32 = 0x22;
pub const BOMB_BLAST_SOUND_VOLUME: f32 = 1.0;
pub const BOMB_SCAN_INTERVAL_MS: u64 = 50;
pub const MAX_OWN_BOMBS: usize = 8;
pub const MAX_OWN_BOMB_HITS: usize = 16;
pub const OWN_BOMB_HIT_MEMORY_MS: u64 = 3_000;
pub const BLAST_NEIGHBOUR_LOG_DISTANCE: f32 = 12.0;
pub const MAX_BLAST_NEIGHBOURS_LOGGED: usize = 12;
