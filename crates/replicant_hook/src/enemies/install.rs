use crate::build_gate::VerifiedBuild;
use crate::enemies::{
    boss_bar_probe, bullet_hit_hook, drop_hook, hit_hook, hold_hook, spawn_hook, word_drop_hook,
};

pub fn install(build: VerifiedBuild) {
    boss_bar_probe::install(build);
    hit_hook::install(build);
    bullet_hit_hook::install(build);
    drop_hook::install(build);
    hold_hook::install(build);
    word_drop_hook::install(build);
    spawn_hook::install(build);
}
