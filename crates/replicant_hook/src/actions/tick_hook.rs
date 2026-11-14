use nc_hook::stall_watch::in_game_thread_step;
use nc_hook::tick_hook::install_game_thread_tick;
use replicant_game::offsets::SCRIPT_MANAGER_TICK_RVA;

use crate::actions::runner::run_queued_actions;
use crate::blocks::tick_blocks;
use crate::bombs::{tick_bombs, tick_walls};
use crate::boot::tick_first_save;
use crate::build_gate::VerifiedBuild;
use crate::carts::tick_carts;
use crate::desync::tick_desync;
use crate::enemies::tick_enemy_sync;
use crate::gimmick_probe::tick_gimmick_probe;
use crate::move_log::tick_move_log;
use crate::net;
use crate::npcs::tick_npc_sync;
use crate::sky::tick_sky_sync;

pub use nc_hook::tick_hook::is_draining;

pub fn install(build: VerifiedBuild) {
    install_game_thread_tick(build, SCRIPT_MANAGER_TICK_RVA, run_game_tick);
}

fn run_game_tick(build: &VerifiedBuild) {
    let steps: [(&'static str, fn(&VerifiedBuild)); 15] = [
        ("script tick", run_queued_actions),
        ("game tick: quit when asked", |_| net::quit_when_asked()),
        ("game tick: readiness", net::report_readiness),
        ("game tick: local snapshot", net::publish_local_snapshot),
        ("game tick: enemy sync", tick_enemy_sync),
        ("game tick: npc sync", tick_npc_sync),
        ("game tick: sky sync", tick_sky_sync),
        ("game tick: desync", tick_desync),
        ("game tick: move log", tick_move_log),
        ("game tick: gimmick probe", tick_gimmick_probe),
        ("game tick: blocks", tick_blocks),
        ("game tick: walls", tick_walls),
        ("game tick: bombs", tick_bombs),
        ("game tick: carts", tick_carts),
        ("game tick: first save", tick_first_save),
    ];
    for (name, step) in steps {
        in_game_thread_step(name, || step(build));
    }
}
