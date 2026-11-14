use replicant_proto::{EnemyId, StoryCallArg};

use crate::kaine::HELD_TRAP_RELEASE_DELAY_MS;
use crate::kaine::companion_off_fights::companion_off_fight_started_by;
use crate::kaine::stand_in_beats::{StandInBeat, stand_in_beat_moved_after};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandInSide {
    Host,
    Guest,
}

/// What one game knows this tick. `beat` is read from the host's flags (the host's own on the
/// host, the guest's copy of them on the guest); `fight_open` is the companion-off window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandInFacts {
    pub side: StandInSide,
    pub in_session: bool,
    pub fight_open: bool,
    pub beat: Option<&'static StandInBeat>,
    pub free_to_move: bool,
    pub now_ms: u64,
}

/// A trap callback the guest's own game popped inside a beat, kept from running until the host's
/// game has run it too.
#[derive(Debug, Clone, PartialEq)]
pub struct HeldTrap {
    pub name: String,
    pub args: Vec<StoryCallArg>,
    pub beat: u8,
    pub host_copy_runs_here: bool,
    pub held_at_ms: u64,
    pub release_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StandInEvent {
    BeatOpened {
        beat: u8,
    },
    /// `catch_up_seen`: one of the beat's catch-up traps ran on this game or the host's game
    /// while it was open. Without it the host's flags moved past the beat by other means (a
    /// story jump, a load), or the host's word is still on its way.
    BeatClosed {
        beat: u8,
        catch_up_seen: bool,
    },
    MoveGuest {
        beat: u8,
        spot: [f32; 3],
        yaw_degrees: f32,
    },
    /// Run the held trap now, as its own game's natural firing.
    Release(HeldTrap),
    Dropped {
        trap: HeldTrap,
        why: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnTrapVerdict {
    Run,
    Hold,
    RunAndTellTheGuest,
}

/// What the once-a-second line of an open beat reports.
#[derive(Debug, Clone, PartialEq)]
pub struct StandInReport {
    pub beat: &'static StandInBeat,
    pub held: Vec<String>,
    pub holds: u32,
    pub traps_told: u32,
    pub hits_scaled: u32,
}

/// The guest's stand-in part in a companion-off fight on one game: which beat is open, whether
/// the guest still has to be moved to its spot, and the trap firings held for the host.
#[derive(Debug, Clone, Default)]
pub struct GuestStandIn {
    open_beat: Option<&'static StandInBeat>,
    move_wanted_for: Option<&'static StandInBeat>,
    catch_up_seen: bool,
    held: Vec<HeldTrap>,
    holds: u32,
    traps_told: u32,
    hits_scaled: u32,
    events: Vec<StandInEvent>,
}

impl GuestStandIn {
    pub const fn new() -> Self {
        Self {
            open_beat: None,
            move_wanted_for: None,
            catch_up_seen: false,
            held: Vec::new(),
            holds: 0,
            traps_told: 0,
            hits_scaled: 0,
            events: Vec::new(),
        }
    }

    pub const fn open_beat(&self) -> Option<&'static StandInBeat> {
        self.open_beat
    }

    pub fn held_traps(&self) -> &[HeldTrap] {
        &self.held
    }

    /// The open beat, when `enemy` is its boss: a guest hit on it is scaled down.
    pub fn scales_hits_on(&self, enemy: EnemyId) -> Option<&'static StandInBeat> {
        self.open_beat.filter(|beat| beat.boss == enemy)
    }

    pub fn note_scaled_hit(&mut self) {
        self.hits_scaled = self.hits_scaled.saturating_add(1);
    }

    /// A script function ran on this game (a pop that was let run, or a trace). A fight start or
    /// retry entry forgets everything held; a copy of a held trap running here makes it moot.
    pub fn function_ran(&mut self, name: &str) {
        if companion_off_fight_started_by(name).is_some() {
            self.drop_all("the fight started again from a start or retry entry");
            self.move_wanted_for = None;
        }
        self.note_catch_up(name);
        let (ran, kept): (Vec<HeldTrap>, Vec<HeldTrap>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|trap| trap.name == name);
        self.held = kept;
        for trap in ran {
            self.events.push(StandInEvent::Dropped {
                trap,
                why: "a copy of it ran here, the held one is not needed",
            });
        }
        if let Some(beat) = stand_in_beat_moved_after(name) {
            self.move_wanted_for = Some(beat);
        }
    }

    /// This game popped `name` itself (not a call the mod queued). On the guest a catch-up trap
    /// of the open beat is held; on the host it runs and the guest is told.
    pub fn own_trap_fired(
        &mut self,
        side: StandInSide,
        in_session: bool,
        name: &str,
        args: Vec<StoryCallArg>,
        now_ms: u64,
    ) -> OwnTrapVerdict {
        let Some(beat) = self
            .open_beat
            .filter(|beat| in_session && beat.held_traps.contains(&name))
        else {
            return OwnTrapVerdict::Run;
        };
        match side {
            StandInSide::Host => {
                self.traps_told = self.traps_told.saturating_add(1);
                OwnTrapVerdict::RunAndTellTheGuest
            }
            StandInSide::Guest => {
                self.holds = self.holds.saturating_add(1);
                if !self.held.iter().any(|trap| trap.name == name) {
                    self.held.push(HeldTrap {
                        name: name.to_owned(),
                        args,
                        beat: beat.beat,
                        host_copy_runs_here: beat.held_traps_the_host_runs_here.contains(&name),
                        held_at_ms: now_ms,
                        release_at_ms: None,
                    });
                }
                OwnTrapVerdict::Hold
            }
        }
    }

    /// The host's game ran `name`: the guest's held copy runs after a short wait, unless the
    /// host's own copy runs here first.
    pub fn host_fired(&mut self, name: &str, now_ms: u64) {
        self.note_catch_up(name);
        for trap in &mut self.held {
            if trap.name == name && trap.release_at_ms.is_none() {
                trap.release_at_ms = Some(now_ms + HELD_TRAP_RELEASE_DELAY_MS);
            }
        }
    }

    pub fn step(&mut self, facts: &StandInFacts) -> Vec<StandInEvent> {
        let beat = facts.beat.filter(|_| facts.fight_open && facts.in_session);
        if beat != self.open_beat {
            if let Some(closed) = self.open_beat {
                self.close(closed, facts.now_ms);
            }
            self.catch_up_seen = false;
            if let Some(opened) = beat {
                self.events
                    .push(StandInEvent::BeatOpened { beat: opened.beat });
            }
            self.open_beat = beat;
        }
        if !facts.fight_open {
            self.drop_all("the fight window closed");
            self.move_wanted_for = None;
        } else if !facts.in_session {
            for trap in &mut self.held {
                trap.release_at_ms = Some(facts.now_ms);
            }
        }
        if facts.side == StandInSide::Guest
            && facts.free_to_move
            && let Some(open) = beat
            && self.move_wanted_for == Some(open)
        {
            self.move_wanted_for = None;
            self.events.push(StandInEvent::MoveGuest {
                beat: open.beat,
                spot: open.spot,
                yaw_degrees: open.yaw_degrees,
            });
        }
        let (due, waiting): (Vec<HeldTrap>, Vec<HeldTrap>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|trap| trap.release_at_ms.is_some_and(|at| at <= facts.now_ms));
        self.held = waiting;
        for trap in due {
            let event = if trap.host_copy_runs_here && facts.in_session {
                StandInEvent::Dropped {
                    trap,
                    why: "the host's copy is sent here instead and has not run yet; yours is not run or sent",
                }
            } else {
                StandInEvent::Release(trap)
            };
            self.events.push(event);
        }
        std::mem::take(&mut self.events)
    }

    pub fn take_report(&mut self) -> Option<StandInReport> {
        let beat = self.open_beat?;
        Some(StandInReport {
            beat,
            held: self.held.iter().map(|trap| trap.name.clone()).collect(),
            holds: std::mem::take(&mut self.holds),
            traps_told: std::mem::take(&mut self.traps_told),
            hits_scaled: std::mem::take(&mut self.hits_scaled),
        })
    }

    fn close(&mut self, closed: &'static StandInBeat, now_ms: u64) {
        self.events.push(StandInEvent::BeatClosed {
            beat: closed.beat,
            catch_up_seen: self.catch_up_seen,
        });
        for trap in &mut self.held {
            if trap.beat == closed.beat && trap.release_at_ms.is_none() {
                // the host has caught up, so it is past every trap of the beat
                trap.release_at_ms = Some(now_ms + HELD_TRAP_RELEASE_DELAY_MS);
            }
        }
        if self.move_wanted_for == Some(closed) {
            self.move_wanted_for = None;
        }
    }

    fn note_catch_up(&mut self, name: &str) {
        if self
            .open_beat
            .is_some_and(|beat| beat.held_traps.contains(&name))
        {
            self.catch_up_seen = true;
        }
    }

    fn drop_all(&mut self, why: &'static str) {
        for trap in std::mem::take(&mut self.held) {
            self.events.push(StandInEvent::Dropped { trap, why });
        }
    }
}
