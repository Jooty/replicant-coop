use crate::kaine::companion_off_fallback::{CompanionOffBossSigns, FallbackWatch};
use crate::kaine::companion_off_fights::{
    CompanionOffFight, companion_off_fight_opened_by, companion_off_fights_approached,
};
use crate::kaine::{COMPANION_LENT_AFTER_A_CALL_MS, COMPANION_LENT_AFTER_A_SCENE_MS, RowOrigin};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionOffStep {
    Leave,
    Deactivate,
    /// The window closed: write her active byte back to what the scripts last left it.
    GiveBack {
        active: bool,
    },
    /// She is lent to the game: write her active byte 1 when `write_active`, and leave her be.
    Lend {
        write_active: bool,
        why: CompanionLendReason,
    },
    /// The lend is over: she is turned off again when `deactivate`.
    LendOver {
        deactivate: bool,
        why: CompanionLendReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionLendReason {
    Scene,
    Call(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionOffEvent {
    Opened { fight: &'static str, by: String },
    Closed { fight: &'static str, by: String },
}

/// One ghost tick as the rule sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionOffTick {
    pub guest_in_session: bool,
    pub active_now: bool,
    pub event_scene: bool,
    pub now_ms: u64,
}

/// A script function that addresses her popped inside the window: write her active byte as
/// below before it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionCallLend {
    pub fight: &'static str,
    pub write_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Lend {
    why: CompanionLendReason,
    until_ms: Option<u64>,
    written: bool,
}

/// The companion-off rule on one game: which fight window is open, what the scripts last left
/// her active byte as, who turned her on since the last report, and whether she is lent back.
#[derive(Debug, Clone, Default)]
pub struct CompanionOff {
    pack: String,
    from_scripts: bool,
    open: Option<&'static CompanionOffFight>,
    holding: bool,
    scripts_left_her_active: Option<bool>,
    last_function: Option<String>,
    turned_on_by: Vec<String>,
    in_scene: bool,
    lend: Option<Lend>,
    fallback: FallbackWatch,
    closed_by_a_fallback_here: Vec<&'static str>,
}

/// What the rule saw since the last report, for the once-a-second line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionOffReport {
    pub fight: &'static str,
    pub turned_on_by: Vec<String>,
    pub lent: Option<CompanionLendReason>,
}

impl CompanionOff {
    pub const fn new() -> Self {
        Self {
            pack: String::new(),
            from_scripts: false,
            open: None,
            holding: false,
            scripts_left_her_active: None,
            last_function: None,
            turned_on_by: Vec::new(),
            in_scene: false,
            lend: None,
            fallback: FallbackWatch::new(),
            closed_by_a_fallback_here: Vec::new(),
        }
    }

    pub fn open_fight(&self) -> Option<&'static str> {
        self.open.map(|fight| fight.name)
    }

    pub const fn holds_her(&self) -> bool {
        self.holding
    }

    pub fn lent_for(&self) -> Option<&CompanionLendReason> {
        self.lend.as_ref().map(|lend| &lend.why)
    }

    /// Whether the rows generated from the scripts take part (off in a new rule). Switching them
    /// off closes a window one of them opened.
    pub fn use_rows_from_scripts(&mut self, enabled: bool) -> Option<CompanionOffEvent> {
        self.from_scripts = enabled;
        let fight = self.open?;
        if enabled || fight.origin != RowOrigin::FromScripts {
            return None;
        }
        self.open = None;
        Some(CompanionOffEvent::Closed {
            fight: fight.name,
            by: "the rows from the scripts being switched off".to_owned(),
        })
    }

    pub const fn uses_rows_from_scripts(&self) -> bool {
        self.from_scripts
    }

    pub fn enter_pack(&mut self, pack: &str) -> Option<CompanionOffEvent> {
        if self.pack == pack {
            return None;
        }
        pack.clone_into(&mut self.pack);
        self.last_function = None;
        self.closed_by_a_fallback_here.clear();
        let fight = self.open.take()?;
        Some(CompanionOffEvent::Closed {
            fight: fight.name,
            by: format!("leaving for {pack}"),
        })
    }

    /// Whether the story flags are worth reading this tick: a window is open (its fight may be
    /// over by the flags), or the current pack holds a fight's approach.
    pub fn story_flags_are_read_here(&self) -> bool {
        self.open.is_some()
            || companion_off_fights_approached(&self.pack, self.from_scripts)
                .next()
                .is_some()
    }

    /// The story flags, read each tick while [`Self::story_flags_are_read_here`]: they open the
    /// window from the area's start while the fight's stretch is on, and close it once the
    /// fight's over flag is on. A row a fallback sign closed in this pack is not opened by them
    /// again until the pack changes.
    pub fn story_flags(&mut self, flag_on: impl Fn(u16) -> bool) -> Option<CompanionOffEvent> {
        if let Some(fight) = self.open {
            let over_flag = fight.over_game_flag().filter(|flag| flag_on(*flag))?;
            self.open = None;
            return Some(CompanionOffEvent::Closed {
                fight: fight.name,
                by: format!(
                    "the story flags (g{over_flag} on: the fight is over, though no end function ran here)"
                ),
            });
        }
        let (fight, flags) = companion_off_fights_approached(&self.pack, self.from_scripts)
            .filter(|fight| !self.closed_by_a_fallback_here.contains(&fight.name))
            .find_map(|fight| {
                let flags = fight.approach.flags?;
                (flag_on(flags.on_game_flag)
                    && !flag_on(flags.off_game_flag)
                    && !fight.over_game_flag().is_some_and(&flag_on))
                .then_some((fight, flags))
            })?;
        self.open_window(fight, false);
        Some(CompanionOffEvent::Opened {
            fight: fight.name,
            by: format!(
                "the story flags (g{} on, g{} off) in {}",
                flags.on_game_flag, flags.off_game_flag, fight.approach.pack
            ),
        })
    }

    /// The boss group whose signs close the open window, if its row has one.
    pub fn boss_group_watched(&self) -> Option<u16> {
        self.open.and_then(|fight| fight.boss_group)
    }

    /// The boss signs of one tick, for a window whose row has a boss group: the boss life gauge
    /// closed for good, the group gone, or no boss long after an entry function close it.
    pub fn boss_signs(&mut self, signs: CompanionOffBossSigns) -> Option<CompanionOffEvent> {
        let fight = self.open?;
        let group = fight.boss_group?;
        let sign = self.fallback.observe(signs)?;
        self.open = None;
        self.closed_by_a_fallback_here.push(fight.name);
        Some(CompanionOffEvent::Closed {
            fight: fight.name,
            by: format!("the fallback sign: {}", sign.text(group)),
        })
    }

    /// A script function ran, with no story flags at hand: only a hand-made row can open.
    pub fn function_ran(&mut self, name: &str) -> Option<CompanionOffEvent> {
        self.function_ran_in_story(name, |_| false)
    }

    /// A script function ran; `flag_on` reads the host's game flags, which a row from the scripts
    /// needs (its on flag) before it opens.
    pub fn function_ran_in_story(
        &mut self,
        name: &str,
        flag_on: impl Fn(u16) -> bool,
    ) -> Option<CompanionOffEvent> {
        self.last_function = Some(name.to_owned());
        if let Some(fight) = self.open {
            if fight.end_functions.contains(&name) {
                self.open = None;
                return Some(CompanionOffEvent::Closed {
                    fight: fight.name,
                    by: name.to_owned(),
                });
            }
            if fight.start_functions.contains(&name) {
                self.fallback.entry_function_ran();
            }
            return None;
        }
        let fight = companion_off_fight_opened_by(name, &self.pack, self.from_scripts, flag_on)?;
        self.open_window(fight, true);
        Some(CompanionOffEvent::Opened {
            fight: fight.name,
            by: name.to_owned(),
        })
    }

    /// After [`Self::function_ran`], at the pop, before `name` runs: when it addresses her inside
    /// an open window with a guest in the session, she is lent to the game for it and a moment
    /// after. `tick` is the game as found at the pop.
    pub fn function_addresses_her(
        &mut self,
        name: &str,
        tick: CompanionOffTick,
    ) -> Option<CompanionCallLend> {
        let fight = self.open?;
        if !tick.guest_in_session || !fight.functions_that_address_her.contains(&name) {
            return None;
        }
        let now_ms = tick.now_ms;
        let write_active = *self.scripts_left_her_active.get_or_insert(tick.active_now);
        let until_ms = now_ms + COMPANION_LENT_AFTER_A_CALL_MS;
        match &mut self.lend {
            Some(lend) if lend.why == CompanionLendReason::Scene && lend.until_ms.is_none() => {}
            Some(lend) => {
                lend.why = CompanionLendReason::Call(name.to_owned());
                lend.until_ms = Some(lend.until_ms.map_or(until_ms, |at| at.max(until_ms)));
                lend.written = true;
            }
            None => {
                self.lend = Some(Lend {
                    why: CompanionLendReason::Call(name.to_owned()),
                    until_ms: Some(until_ms),
                    written: true,
                });
            }
        }
        Some(CompanionCallLend {
            fight: fight.name,
            write_active,
        })
    }

    /// One tick. While a window is open and a guest is in the session she is turned off, except
    /// during an event scene and a moment after it, or after a call that addresses her; once the
    /// window closes (or the guest leaves) she is given back.
    pub fn step(&mut self, tick: CompanionOffTick) -> CompanionOffStep {
        if self.open.is_some() && tick.guest_in_session {
            if let Some(step) = self.lend_step(tick) {
                return step;
            }
            if self.scripts_left_her_active.is_none() {
                self.scripts_left_her_active = Some(tick.active_now);
            } else if tick.active_now {
                self.scripts_left_her_active = Some(true);
                let by = self
                    .last_function
                    .clone()
                    .unwrap_or_else(|| "no script function seen".to_owned());
                self.turned_on_by.push(by);
            }
            if tick.active_now {
                self.holding = true;
                return CompanionOffStep::Deactivate;
            }
            return CompanionOffStep::Leave;
        }
        self.lend = None;
        self.in_scene = false;
        let was_holding = self.holding;
        let scripts_left = self.scripts_left_her_active.take();
        self.holding = false;
        if !was_holding || tick.active_now {
            // never held, or the game turned her on itself after the window: it has her already
            return CompanionOffStep::Leave;
        }
        CompanionOffStep::GiveBack {
            active: scripts_left.unwrap_or(true),
        }
    }

    pub fn take_report(&mut self) -> Option<CompanionOffReport> {
        let fight = self.open?;
        Some(CompanionOffReport {
            fight: fight.name,
            turned_on_by: std::mem::take(&mut self.turned_on_by),
            lent: self.lend.as_ref().map(|lend| lend.why.clone()),
        })
    }

    fn open_window(&mut self, fight: &'static CompanionOffFight, by_an_entry_function: bool) {
        self.open = Some(fight);
        self.fallback = FallbackWatch::new();
        if by_an_entry_function {
            self.fallback.entry_function_ran();
        }
    }

    fn lend_step(&mut self, tick: CompanionOffTick) -> Option<CompanionOffStep> {
        if tick.event_scene && !self.in_scene {
            self.in_scene = true;
            self.lend = Some(Lend {
                why: CompanionLendReason::Scene,
                until_ms: None,
                written: false,
            });
        } else if !tick.event_scene && self.in_scene {
            self.in_scene = false;
            if let Some(lend) = &mut self.lend {
                let until_ms = tick.now_ms + COMPANION_LENT_AFTER_A_SCENE_MS;
                lend.until_ms = Some(lend.until_ms.map_or(until_ms, |at| at.max(until_ms)));
            }
        }
        let lend = self.lend.as_mut()?;
        let scripts_left = *self.scripts_left_her_active.get_or_insert(tick.active_now);
        if lend.until_ms.is_some_and(|until| tick.now_ms >= until) {
            let why = lend.why.clone();
            self.lend = None;
            if tick.active_now {
                self.holding = true;
            }
            return Some(CompanionOffStep::LendOver {
                deactivate: tick.active_now,
                why,
            });
        }
        if !lend.written {
            lend.written = true;
            let write_active = scripts_left && !tick.active_now;
            return Some(CompanionOffStep::Lend {
                write_active,
                why: lend.why.clone(),
            });
        }
        // while she is lent, what the game does to her is what the scripts left her as
        self.scripts_left_her_active = Some(tick.active_now);
        Some(CompanionOffStep::Leave)
    }
}
