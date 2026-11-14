use eframe::egui::{Id, Rect, Ui, pos2};
use nc_launcher_core::{
    ART_FADE_MS, Easing, HATCH_DRIFT_MS_PER_PX, SEAT_ENTER_MS, SEAT_FIRST_DELAY_MS,
    SEAT_STAGGER_MS, SeatPicture, drift_offset_px, seat_pictures,
};

use crate::app::Launcher;
use crate::app::motion::{changed_since, entering, page_section, page_since, with_clock};
use crate::app::pages::players_heading::{PLAYERS_HEADING_HEIGHT_PX, players_heading};
use crate::app::pages::seat::{Seat, SeatName, draw_seat};
use crate::app::pages::seat_ledger::{CharacterCell, PingCell, SeatLedger};
use crate::app::parts::HATCH_STEP_ALONG_PX;
use crate::home::{BannerArtState, bundled_portrait};
use crate::lobby::{
    PlayerRole, PlayerRow, RowInputs, body_label, character_id_for_body, player_rows,
};

const HEADING_TO_SEATS_GAP_PX: f32 = 14.0;
const SEAT_GAP_PX: f32 = 28.0;
const SEAT_OCCUPANT_SALT: &str = "seat-occupant";

/// What the seats need from the session: who the partner is, and what the open seat says.
pub(super) struct SeatFacts<'a> {
    pub peer_name: Option<&'a str>,
    pub open_seat_name: &'static str,
    pub open_seat_waiting: bool,
}

pub(super) fn draw_player_seats(ui: &mut Ui, launcher: &mut Launcher, facts: SeatFacts) {
    let main = ui.max_rect();
    let viewer_is_guest = launcher.link.guest && facts.peer_name.is_some();
    let party = 1 + launcher.link.members.len();
    let max_players = launcher.link.rules.max_players;
    page_section(ui, 1, |ui| players_heading(ui, party, max_players));
    let seats_top = main.top() + PLAYERS_HEADING_HEIGHT_PX + HEADING_TO_SEATS_GAP_PX;
    let seat_width = ((main.width() - SEAT_GAP_PX) / 2.0).max(0.0);
    let own_seat = launcher.own_seat_name();
    let own_name = own_seat.name.clone();
    let rows = player_rows(RowInputs {
        viewer_is_guest,
        own_name: &own_name,
        peer_name: facts.peer_name,
    });
    let art = match launcher
        .banner_art
        .state(ui.ctx(), launcher.profile.game_id)
    {
        BannerArtState::Ready(art) => Some(art.colour),
        BannerArtState::Pending | BannerArtState::Missing => None,
    };
    let roles = [PlayerRole::Host, PlayerRole::Guest];
    let profile = launcher.profile;
    let characters: Vec<Option<&str>> = roles
        .iter()
        .map(|role| {
            rows.iter()
                .find(|row| row.role == *role)
                .and_then(|row| character_id_for_body(row.body))
        })
        .collect();
    let pictures = seat_pictures(profile, &characters, |id| {
        bundled_portrait(profile.game_id, id).is_some()
    });
    let hatch_shift_px = hatch_drift(ui, facts.open_seat_waiting);
    for (index, role) in roles.into_iter().enumerate() {
        let rect = Rect::from_min_max(
            pos2(
                main.left() + index as f32 * (seat_width + SEAT_GAP_PX),
                seats_top,
            ),
            pos2(
                main.left() + index as f32 * (seat_width + SEAT_GAP_PX) + seat_width,
                main.bottom(),
            ),
        );
        let row = rows.iter().find(|row| row.role == role);
        let (since, delay_ms) = seat_entrance(ui, index, row);
        let art_shown = with_clock(ui, |clock, _| {
            clock.entrance(
                since,
                delay_ms + SEAT_ENTER_MS,
                ART_FADE_MS,
                Easing::EaseOut,
            )
        });
        let ledger = match row {
            Some(row) => ledger_for(launcher, row),
            None => open_ledger(),
        };
        let name = match row {
            Some(row) if row.you && own_seat.editable => {
                SeatName::Typed(&mut launcher.new_game_name)
            }
            Some(row) if row.you => SeatName::Fixed(&own_seat.name),
            Some(row) => SeatName::Fixed(&row.name),
            None => SeatName::Open(facts.open_seat_name),
        };
        let seat = Seat {
            number: index + 1,
            role: role.text(),
            you: row.is_some_and(|row| row.you),
            name,
            ledger,
            art_shown,
            hatch_shift_px,
        };
        let picture = match pictures[index] {
            SeatPicture::Portrait { character_id, .. } => {
                launcher
                    .portrait_art
                    .texture(ui.ctx(), profile.game_id, character_id)
            }
            SeatPicture::KeyArt(_) => art.clone(),
        };
        let crop = pictures[index].crop();
        entering(ui, since, delay_ms, SEAT_ENTER_MS, |ui| {
            draw_seat(ui, rect, seat, picture.as_ref(), crop)
        });
    }
}

fn seat_entrance(ui: &Ui, index: usize, row: Option<&PlayerRow>) -> (f64, f32) {
    // Your own seat keeps its key while you type your name; anyone else's changes with who sits there.
    let occupant = row.map(|row| (row.you, if row.you { "" } else { row.name.as_str() }));
    let filled_since = changed_since(ui, Id::unique((SEAT_OCCUPANT_SALT, index)), occupant);
    let page = page_since(ui);
    if filled_since > page {
        return (filled_since, 0.0);
    }
    (page, SEAT_FIRST_DELAY_MS + index as f32 * SEAT_STAGGER_MS)
}

fn hatch_drift(ui: &Ui, waiting: bool) -> f32 {
    if !waiting {
        return 0.0;
    }
    let focused = ui.input(|input| input.focused);
    with_clock(ui, |clock, _| {
        if focused {
            clock.drift_px(HATCH_DRIFT_MS_PER_PX, HATCH_STEP_ALONG_PX)
        } else if clock.motion() {
            // Unfocused, it moves only when the window repaints anyway.
            drift_offset_px(clock.now(), HATCH_DRIFT_MS_PER_PX, HATCH_STEP_ALONG_PX)
        } else {
            0.0
        }
    })
}

fn ledger_for(launcher: &mut Launcher, row: &PlayerRow) -> SeatLedger {
    let character = CharacterCell::Fixed(body_label(row.body).to_owned());
    let status = if row.you {
        launcher.your_player_status()
    } else {
        launcher.their_player_status()
    };
    let ping = if row.you {
        PingCell::Local
    } else {
        launcher
            .link
            .partner()
            .and_then(|partner| partner.rtt_ms)
            .or_else(|| {
                launcher
                    .link
                    .rtt
                    .map(|rtt| u32::try_from(rtt.as_millis()).unwrap_or(u32::MAX))
            })
            .map_or(PingCell::Unknown, PingCell::Millis)
    };
    SeatLedger {
        character,
        status: Some(status),
        ping,
    }
}

fn open_ledger() -> SeatLedger {
    SeatLedger {
        character: CharacterCell::Empty,
        status: None,
        ping: PingCell::Unknown,
    }
}
