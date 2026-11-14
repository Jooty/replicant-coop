use replicant_game::new_game::first_save_permitted;
use replicant_game::save_files::CoopSaveName;

use crate::boot;
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::game_hooks::{
    cursor_calls, door_gate, focus_menu_block, invincibility, menu_free_calls, menu_no_pause,
    mouse_block, noclip, popup_saved_mask, quiet_end, window_focus, zone_gate,
};
use crate::net::AutoConfig;
use crate::player_names;
use crate::saves;

pub fn install(build: VerifiedBuild) {
    invincibility::install(build);
    noclip::install(build);
    zone_gate::install(build);
    menu_no_pause::install(build);
    popup_saved_mask::install(build);
    menu_free_calls::install(build);
    door_gate::install(build);
    crate::render_camera::install(build);
    cursor_calls::install();
    mouse_block::install();
    player_names::install(build);
    match AutoConfig::load_beside_game() {
        Ok(Some(config)) => {
            let mut veto_writes = config.block_saves;
            if config.link.is_some() {
                match CoopSaveName::parse(config.coop_save_file.as_deref()) {
                    Ok(name) => saves::install_coop_files(build, name, !config.block_saves),
                    Err(refusal) => {
                        log_line(&format!(
                            "saves: the link file names the co-op file {:?}, refused ({refusal}); \
                             co-op saving is off and no save is written this session",
                            config.coop_save_file.as_deref().unwrap_or_default()
                        ));
                        veto_writes = true;
                    }
                }
                crate::effects::install(build);
                crate::npcs::install(build);
                crate::music::install(build);
                crate::minimap::install(build);
                crate::pickups::install(build);
                quiet_end::install(build);
                crate::containers::install(build);
                crate::dialogue::install(build);
                crate::story_writes::install(build);
                crate::story_relay::install(build);
                crate::fight_sync::install(build);
                crate::script_rolls::install(build);
                crate::story_jump::install(build);
                crate::downed::install(build);
                crate::puppet::install_book_native(build);
                crate::boss_messages::install(build);
                crate::rewind::install(build);
                crate::fade_sync::install(build);
                crate::cutscene_skip::install(build);
                crate::enemies::install(build);
                crate::enemy_bullets::install(build);
                crate::enemy_bullets::install_script_commands(build);
                crate::blocks::install(build);
                crate::bombs::install(build);
                crate::carts::install(build);
                crate::attack_gauge::install(build);
                crate::player_body::install(build);
            }
            if veto_writes {
                saves::install(build);
            }
            if config.keep_running || config.link.is_some() {
                window_focus::install(build, config.keep_running);
            }
            if config.dev_no_focus_menu {
                focus_menu_block::install(build);
            }
            if let Some(slot) = config.boot_slot {
                boot::install(build, slot, config.link.is_some());
                if config.new_game_slot.is_some() {
                    log_line(
                        "hooks: boot_slot and new_game_slot are both set, new_game_slot is ignored",
                    );
                }
            } else if let Some(slot) = config.new_game_slot {
                let name = config.new_game_name.as_deref().or(config.name.as_deref());
                let linked = config.link.is_some();
                let first_save = first_save_permitted(linked, config.block_saves);
                boot::install_new_game(build, slot, name, linked, first_save);
            }
        }
        Ok(None) => {}
        Err(error) => log_line(&format!("hooks: {error}")),
    }
}
