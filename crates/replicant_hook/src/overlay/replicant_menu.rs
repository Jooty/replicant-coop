use nc_hook::menu::{
    AreaChangeRefusal, AreaEntry, CoOpStatus, ExtraPanel, FixEntry, FixOutcome, GameMenu, MenuNote,
    SectionKind, StoryJumpEntry,
};
use replicant_game::marker::read_world_flags;
use replicant_game::stay_together::FollowRole;
use replicant_game::{PHASE_TABLE, PhaseEntry};
use replicant_proto::StoryJumpPoint;

use crate::actions::{self, Action};
use crate::net::{self, NetState};
use crate::overlay::co_op_lines::{co_op_notes, co_op_status};
use crate::overlay::fix_actions::{fix_entries, fix_notes, run_fix};
use crate::overlay::replicant_overlay::{
    AREA_POINT_PANEL_ID, CO_OP_PANEL_ID, DIAGNOSTICS_PANEL_ID, ReplicantOverlay, TRAINER_PANEL_ID,
};
use crate::overlay::status_lines::game_status_lines;
use crate::story_jump::{host_can_offer_jump, jump_is_waiting_for_menu, last_refusal};
use crate::teleport::last_outcome as last_teleport_outcome;

const STORY_JUMPS: [(StoryJumpPoint, &str); 11] = [
    (StoryJumpPoint::AfterPrologue, "Skip the prologue"),
    (StoryJumpPoint::AtPopolaTalk, "Library: talk to Popola"),
    (
        StoryJumpPoint::BeforeNextDay,
        "Village: before the next day",
    ),
    (
        StoryJumpPoint::AtShrineEntrance,
        "Lost Shrine: the entrance",
    ),
    (StoryJumpPoint::AtShrineTop, "Shrine top: the stone field"),
    (StoryJumpPoint::AtShrineBoss, "Shrine boss: the arena"),
    (StoryJumpPoint::BossFight3, "Boss fight 3"),
    (StoryJumpPoint::JunkHeapFloorB2, "Junk Heap: floor B2"),
    (
        StoryJumpPoint::JunkHeapBossFight5,
        "Junk Heap: boss fight 5",
    ),
    (StoryJumpPoint::BossFight6, "Boss fight 6"),
    (StoryJumpPoint::AfterBossFight6, "After boss fight 6"),
];

impl GameMenu for ReplicantOverlay {
    fn co_op_status(&self) -> CoOpStatus {
        co_op_status()
    }

    fn areas(&self) -> Vec<AreaEntry> {
        PHASE_TABLE
            .iter()
            .enumerate()
            .map(|(index, entry)| AreaEntry {
                id: u32::try_from(index).unwrap_or(u32::MAX),
                name: format!("{}  ({})", entry.name, entry.area),
            })
            .collect()
    }

    fn current_area(&self) -> Option<AreaEntry> {
        let phase = self.game.as_ref()?.current_phase().ok()?;
        let index = PhaseEntry::index_of(&phase)?;
        let entry = PHASE_TABLE.get(index)?;
        Some(AreaEntry {
            id: u32::try_from(index).ok()?,
            name: format!("{}  ({})", entry.name, entry.area),
        })
    }

    fn area_change_blocker(&self) -> Option<AreaChangeRefusal> {
        if let Some(net) = net::handle() {
            let view = net.view();
            if view.state == NetState::Connected
                && FollowRole::from_slot(view.local_slot) != FollowRole::Host
            {
                return Some(AreaChangeRefusal::NotHost);
            }
        }
        let flags = read_world_flags(self.game.as_ref()?.memory()).ok()?;
        if flags.event_scene {
            Some(AreaChangeRefusal::SceneRunning)
        } else if flags.talk_window_open {
            Some(AreaChangeRefusal::TalkRunning)
        } else if flags.main_menu_open || flags.world_frozen() {
            Some(AreaChangeRefusal::PopUpRunning)
        } else {
            None
        }
    }

    fn change_area(&self, area: &AreaEntry) -> Result<(), String> {
        let entry = usize::try_from(area.id)
            .ok()
            .and_then(|index| PHASE_TABLE.get(index))
            .ok_or_else(|| format!("unknown area {}", area.id))?;
        let action =
            Action::change_phase(entry.name, self.area_point).map_err(|e| e.to_string())?;
        actions::enqueue(action).map_err(|error| error.to_string())
    }

    fn go_to_partner(&self) -> Result<String, String> {
        actions::enqueue(Action::GoToGuest)
            .map(|()| String::from("asked the game to go to the guest"))
            .map_err(|error| format!("not queued: {error}"))
    }

    fn bring_partner(&self) -> Result<String, String> {
        actions::enqueue(Action::BringGuestHere)
            .map(|()| String::from("asked the game to bring the guest"))
            .map_err(|error| format!("not queued: {error}"))
    }

    fn story_jumps(&self) -> Vec<StoryJumpEntry> {
        if !host_can_offer_jump() {
            return Vec::new();
        }
        STORY_JUMPS
            .iter()
            .enumerate()
            .map(|(index, (_, label))| StoryJumpEntry {
                id: u32::try_from(index).unwrap_or(u32::MAX),
                label: (*label).to_owned(),
            })
            .collect()
    }

    fn run_story_jump(&self, id: u32) -> Result<String, String> {
        let (point, _) = usize::try_from(id)
            .ok()
            .and_then(|index| STORY_JUMPS.get(index))
            .ok_or_else(|| format!("unknown story jump {id}"))?;
        actions::enqueue(Action::StoryJump(*point))
            .map(|()| String::from("asked the game to jump"))
            .map_err(|error| format!("not queued: {error}"))
    }

    fn fixes(&self) -> Vec<FixEntry> {
        fix_entries()
    }

    fn run_fix(&self, id: u32) -> FixOutcome {
        run_fix(id)
    }

    fn status_lines(&self) -> Vec<String> {
        game_status_lines(self.game.as_ref())
    }

    fn extra_panels(&self) -> Vec<ExtraPanel> {
        vec![
            ExtraPanel {
                id: AREA_POINT_PANEL_ID,
                label: "area point",
            },
            ExtraPanel {
                id: TRAINER_PANEL_ID,
                label: "trainer",
            },
            ExtraPanel {
                id: DIAGNOSTICS_PANEL_ID,
                label: "diagnostics",
            },
            ExtraPanel {
                id: CO_OP_PANEL_ID,
                label: "co-op details",
            },
        ]
    }

    fn section_notes(&self, section: SectionKind) -> Vec<MenuNote> {
        match section {
            SectionKind::CoOp => co_op_notes(),
            SectionKind::Teleport => teleport_notes(),
            SectionKind::StoryJump => story_jump_notes(),
            SectionKind::Fixes => fix_notes(),
            SectionKind::SettingsAndLogs => Vec::new(),
        }
    }
}

fn teleport_notes() -> Vec<MenuNote> {
    let mut notes = Vec::new();
    if !actions::is_draining() {
        notes.push(MenuNote::plain(
            "game-thread hook not installed: selecting does nothing",
        ));
    }
    let outcome = last_teleport_outcome();
    if !outcome.is_empty() {
        notes.push(MenuNote::plain(outcome));
    }
    notes
}

fn story_jump_notes() -> Vec<MenuNote> {
    let mut notes = Vec::new();
    if !host_can_offer_jump() {
        notes.push(MenuNote::plain("only a connected host can jump"));
    }
    if jump_is_waiting_for_menu() {
        notes.push(MenuNote::plain(
            "queued: close the game menu and the jump runs",
        ));
    }
    let refusal = last_refusal();
    if !refusal.is_empty() {
        notes.push(MenuNote::plain(format!("refused: {refusal}")));
    }
    notes
}
