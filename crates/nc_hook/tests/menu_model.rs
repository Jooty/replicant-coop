use nc_hook::menu::{
    AreaChangeRefusal, AreaEntry, CoOpStatus, ExtraPanel, FixEntry, FixOutcome, GameMenu, MenuNote,
    PlayerLine, SECTION_ORDER, SectionBody, SectionKind, StoryJumpEntry, build_menu,
};

#[derive(Default)]
struct MadeUpGame {
    players: Vec<PlayerLine>,
    areas: Vec<AreaEntry>,
    jumps: Vec<StoryJumpEntry>,
    fixes: Vec<FixEntry>,
    lines: Vec<String>,
    panels: Vec<ExtraPanel>,
    teleport_notes: Vec<MenuNote>,
}

impl GameMenu for MadeUpGame {
    fn co_op_status(&self) -> CoOpStatus {
        CoOpStatus {
            link: "connected".to_owned(),
            connected: true,
            players: self.players.clone(),
            ping_ms: Some(40),
        }
    }

    fn areas(&self) -> Vec<AreaEntry> {
        self.areas.clone()
    }

    fn current_area(&self) -> Option<AreaEntry> {
        self.areas.first().cloned()
    }

    fn area_change_blocker(&self) -> Option<AreaChangeRefusal> {
        None
    }

    fn change_area(&self, _area: &AreaEntry) -> Result<(), String> {
        Ok(())
    }

    fn go_to_partner(&self) -> Result<String, String> {
        Ok(String::from("going"))
    }

    fn bring_partner(&self) -> Result<String, String> {
        Ok(String::from("bringing"))
    }

    fn story_jumps(&self) -> Vec<StoryJumpEntry> {
        self.jumps.clone()
    }

    fn run_story_jump(&self, _id: u32) -> Result<String, String> {
        Ok(String::from("jumped"))
    }

    fn fixes(&self) -> Vec<FixEntry> {
        self.fixes.clone()
    }

    fn run_fix(&self, _id: u32) -> FixOutcome {
        FixOutcome::Done
    }

    fn status_lines(&self) -> Vec<String> {
        self.lines.clone()
    }

    fn extra_panels(&self) -> Vec<ExtraPanel> {
        self.panels.clone()
    }

    fn section_notes(&self, section: SectionKind) -> Vec<MenuNote> {
        match section {
            SectionKind::Teleport => self.teleport_notes.clone(),
            _ => Vec::new(),
        }
    }
}

fn full_game() -> MadeUpGame {
    MadeUpGame {
        players: vec![PlayerLine {
            name: "Ada".to_owned(),
            detail: "same area".to_owned(),
        }],
        areas: vec![
            AreaEntry {
                id: 1,
                name: "Meadow".to_owned(),
            },
            AreaEntry {
                id: 2,
                name: "Cave".to_owned(),
            },
        ],
        jumps: vec![StoryJumpEntry {
            id: 7,
            label: "Chapter two".to_owned(),
        }],
        fixes: vec![FixEntry {
            id: 3,
            label: "Resync".to_owned(),
            disabled_reason: None,
        }],
        lines: vec!["hp: 10".to_owned()],
        panels: vec![ExtraPanel {
            id: 1,
            label: "trainer",
        }],
        teleport_notes: Vec::new(),
    }
}

#[test]
fn every_game_gets_the_same_five_sections_in_the_same_order() {
    let order = [
        SectionKind::CoOp,
        SectionKind::Teleport,
        SectionKind::StoryJump,
        SectionKind::Fixes,
        SectionKind::SettingsAndLogs,
    ];
    assert_eq!(SECTION_ORDER, order);
    for game in [full_game(), MadeUpGame::default()] {
        let kinds: Vec<SectionKind> = build_menu(&game)
            .sections
            .iter()
            .map(|section| section.kind)
            .collect();
        assert_eq!(kinds, order);
    }
}

#[test]
fn the_section_titles_are_the_ones_the_player_reads() {
    let titles: Vec<&str> = SECTION_ORDER.iter().map(|kind| kind.title()).collect();
    assert_eq!(
        titles,
        [
            "Co-op",
            "Teleport",
            "Story jump",
            "Fixes",
            "Settings and logs"
        ]
    );
}

#[test]
fn a_game_with_entries_has_no_empty_notes() {
    let menu = build_menu(&full_game());
    assert!(
        menu.sections
            .iter()
            .all(|section| section.empty_note.is_none())
    );
}

#[test]
fn empty_lists_stay_in_the_menu_and_say_so() {
    let menu = build_menu(&MadeUpGame::default());
    assert_eq!(menu.sections.len(), 5);
    for section in &menu.sections {
        assert!(
            section.empty_note.is_some(),
            "{:?} should explain that it is empty",
            section.kind
        );
    }
}

#[test]
fn the_teleport_section_carries_the_areas_and_the_current_one() {
    let menu = build_menu(&full_game());
    let SectionBody::Teleport { current, areas } = &menu.sections[1].body else {
        panic!("second section is the teleport section");
    };
    assert_eq!(areas.len(), 2);
    assert_eq!(current.as_ref().map(|area| area.id), Some(1));
}

#[test]
fn a_game_may_add_notes_to_a_section() {
    let mut game = full_game();
    game.teleport_notes = vec![MenuNote::warn("careful")];
    let menu = build_menu(&game);
    assert_eq!(menu.sections[1].notes, vec![MenuNote::warn("careful")]);
    assert!(menu.sections[0].notes.is_empty());
}

#[test]
fn a_settings_section_with_only_panels_is_not_empty() {
    let game = MadeUpGame {
        panels: vec![ExtraPanel {
            id: 1,
            label: "debug",
        }],
        ..MadeUpGame::default()
    };
    let menu = build_menu(&game);
    assert!(menu.sections[4].empty_note.is_none());
}
