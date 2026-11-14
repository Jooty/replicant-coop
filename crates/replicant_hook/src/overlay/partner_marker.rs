use std::sync::atomic::{AtomicBool, Ordering};

use hudhook::imgui::Ui;
use replicant_game::Replicant;
use replicant_game::downed::{PartnerBadge, PartnerBadgeInputs, partner_badge};
use replicant_game::marker::{
    HEAD_OFFSET_UNITS, MarkerVisibility, Projected, ProjectionMethod, fade_alpha, hidden_reason,
    marker_label, project, read_camera_state, read_world_flags,
};

use crate::downed::downed_status;
use crate::memory::InProcessMemory;
use crate::net::{self, NetShared};
use crate::overlay::hud_assets;
use crate::render_camera::{
    FrameSource, frame_for_marker, log_summary_if_due, note_marker_source, note_position_source,
};

const TAG_COLOR: [f32; 4] = [1.0, 0.92, 0.55, 1.0];
const SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.85];
const ARROW_COLOR: [f32; 4] = [1.0, 0.85, 0.25, 0.95];
const TAG_LIFT_PIXELS: f32 = 6.0;
const ARROW_HALF_LENGTH_PIXELS: f32 = 16.0;
const ARROW_HALF_WIDTH_PIXELS: f32 = 11.0;
const LABEL_GAP_PIXELS: f32 = 22.0;
const SHADOW_OFFSET_PIXELS: f32 = 1.0;
const TAG_TEXT_SIZE_PIXELS: f32 = 20.0;
const DOWNED_COLOR: [f32; 4] = [1.0, 0.35, 0.3, 1.0];
const REVIVE_BAR_FILL_COLOR: [f32; 4] = [0.45, 0.92, 0.5, 0.95];
const REVIVE_BAR_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.6];
const DOWNED_TEXT: &str = "DOWNED";
const DOWNED_TEXT_SIZE_PIXELS: f32 = 16.0;
const REVIVE_BAR_WIDTH_PIXELS: f32 = 90.0;
const REVIVE_BAR_HEIGHT_PIXELS: f32 = 8.0;
const REVIVE_BAR_BORDER_PIXELS: f32 = 1.0;
const BADGE_GAP_PIXELS: f32 = 2.0;

static MARKER_ENABLED: AtomicBool = AtomicBool::new(true);
static MATRIX_METHOD: AtomicBool = AtomicBool::new(false);

pub(super) struct ShownPartner {
    name: String,
    pub(super) position: [f32; 3],
    distance: f32,
    pub(super) camera: replicant_game::marker::CameraState,
}

pub(super) fn marker_enabled() -> bool {
    MARKER_ENABLED.load(Ordering::Relaxed)
}

pub(super) fn set_marker_enabled(enabled: bool) {
    MARKER_ENABLED.store(enabled, Ordering::Relaxed);
}

pub(super) fn matrix_method() -> bool {
    MATRIX_METHOD.load(Ordering::Relaxed)
}

pub(super) fn set_matrix_method(enabled: bool) {
    MATRIX_METHOD.store(enabled, Ordering::Relaxed);
}

pub(super) fn draw_partner_marker(ui: &Ui, game: Option<&Replicant<InProcessMemory>>) {
    log_summary_if_due();
    if !marker_enabled() {
        return;
    }
    let (Some(game), Some(net)) = (game, net::handle()) else {
        return;
    };
    let view = net.view();
    if !view.options.show_names {
        return;
    }
    let Some(shown) = shown_partner(&view, game) else {
        return;
    };
    let [width, height] = ui.io().display_size;
    let method = if matrix_method() {
        ProjectionMethod::WorldMatrix
    } else {
        ProjectionMethod::ViewProjection
    };
    let head = [
        shown.position[0],
        shown.position[1] + HEAD_OFFSET_UNITS,
        shown.position[2],
    ];
    let projected = project(&shown.camera, method, head, width, height);
    let alpha = fade_alpha(shown.distance);
    let ping_ms = view
        .rtt
        .filter(|_| view.options.show_ping)
        .map(|rtt| rtt.as_millis().min(u128::from(u32::MAX)) as u32);
    let mut label = marker_label(
        &shown.name,
        view.options.show_distance.then_some(shown.distance),
        ping_ms,
    );
    if let Some(note) = view
        .partner_state()
        .and_then(|remote| remote.state.activity.label())
    {
        label = format!("{label} \u{b7} {note}");
    }
    let status = downed_status();
    let badge = partner_badge(PartnerBadgeInputs {
        session_active: status.session_active,
        local: status.local,
        partner: status.partner,
        game_over: status.game_over,
        reviving_partner_progress: status.revive_progress,
        being_revived_progress: status.incoming_revive_progress,
    });
    match projected {
        Projected::OnScreen { x, y } => draw_tag(ui, &label, badge, x, y, alpha),
        Projected::EdgeArrow {
            x,
            y,
            angle_radians,
        } => draw_arrow(
            ui,
            &label,
            badge,
            [x, y],
            angle_radians,
            [width, height],
            alpha,
        ),
        Projected::Invalid => {}
    }
}

pub(super) fn shown_partner(
    view: &NetShared,
    game: &Replicant<InProcessMemory>,
) -> Option<ShownPartner> {
    let remote = view.partner_state();
    let presented = frame_for_marker();
    let camera = presented
        .map(|(frame, _)| frame.camera)
        .or_else(|| read_camera_state(game.memory()).ok().flatten());
    note_marker_source(presented.map_or(FrameSource::LiveRead, |(_, source)| source));
    let flags = read_world_flags(game.memory()).ok()?;
    let visibility = MarkerVisibility {
        enabled: true,
        peer_known: remote.is_some(),
        same_phase: remote
            .zip(view.local.as_ref())
            .is_some_and(|(remote, local)| remote.state.phase == local.phase),
        peer_state_age_ms: remote.map_or(0, |remote| remote.age().as_millis() as u64),
        camera,
        event_scene: flags.event_scene,
        world_frozen: flags.world_frozen(),
    };
    if hidden_reason(&visibility).is_some() {
        return None;
    }
    let (remote, local, camera) = (remote?, view.local.as_ref()?, camera?);
    let squared: f32 = remote
        .state
        .position
        .iter()
        .zip(local.position)
        .map(|(a, b)| (a - b) * (a - b))
        .sum();
    let drawn_position = presented.and_then(|(frame, _)| frame.puppet_position);
    note_position_source(drawn_position.is_some());
    Some(ShownPartner {
        name: remote.name.clone(),
        position: drawn_position.unwrap_or(remote.state.position),
        distance: squared.sqrt(),
        camera,
    })
}

fn draw_tag(ui: &Ui, label: &str, badge: PartnerBadge, x: f32, y: f32, alpha: f32) {
    let size = hud_assets::text_size(ui, label, TAG_TEXT_SIZE_PIXELS);
    let badge_height = badge_height(badge);
    let label_at = [
        x - size[0] * 0.5,
        y - size[1] - TAG_LIFT_PIXELS - badge_height,
    ];
    draw_text(ui, label, label_at, alpha);
    draw_badge(
        ui,
        badge,
        x,
        label_at[1] + size[1] + BADGE_GAP_PIXELS,
        alpha,
    );
}

fn badge_height(badge: PartnerBadge) -> f32 {
    match badge {
        PartnerBadge::Hidden => 0.0,
        PartnerBadge::Downed => DOWNED_TEXT_SIZE_PIXELS + BADGE_GAP_PIXELS,
        PartnerBadge::Reviving(_) => REVIVE_BAR_HEIGHT_PIXELS + BADGE_GAP_PIXELS,
    }
}

fn draw_badge(ui: &Ui, badge: PartnerBadge, center_x: f32, top: f32, alpha: f32) {
    match badge {
        PartnerBadge::Hidden => {}
        PartnerBadge::Downed => {
            let size = hud_assets::text_size(ui, DOWNED_TEXT, DOWNED_TEXT_SIZE_PIXELS);
            hud_assets::draw_text(
                &ui.get_background_draw_list(),
                DOWNED_TEXT,
                [center_x - size[0] * 0.5, top],
                DOWNED_TEXT_SIZE_PIXELS,
                faded(DOWNED_COLOR, alpha),
                (faded(SHADOW_COLOR, alpha), SHADOW_OFFSET_PIXELS),
            );
        }
        PartnerBadge::Reviving(progress) => draw_revive_bar(ui, progress, center_x, top, alpha),
    }
}

fn draw_revive_bar(ui: &Ui, progress: f32, center_x: f32, top: f32, alpha: f32) {
    let left = center_x - REVIVE_BAR_WIDTH_PIXELS * 0.5;
    let right = left + REVIVE_BAR_WIDTH_PIXELS;
    let bottom = top + REVIVE_BAR_HEIGHT_PIXELS;
    let draw_list = ui.get_background_draw_list();
    draw_list
        .add_rect(
            [left, top],
            [right, bottom],
            faded(REVIVE_BAR_BACKGROUND_COLOR, alpha),
        )
        .filled(true)
        .build();
    let inner = REVIVE_BAR_BORDER_PIXELS;
    let fill_right = left + inner + (REVIVE_BAR_WIDTH_PIXELS - inner * 2.0) * progress;
    draw_list
        .add_rect(
            [left + inner, top + inner],
            [fill_right, bottom - inner],
            faded(REVIVE_BAR_FILL_COLOR, alpha),
        )
        .filled(true)
        .build();
    draw_list
        .add_rect([left, top], [right, bottom], faded(TAG_COLOR, alpha))
        .thickness(REVIVE_BAR_BORDER_PIXELS)
        .build();
}

fn draw_text(ui: &Ui, label: &str, at: [f32; 2], alpha: f32) {
    hud_assets::draw_text(
        &ui.get_background_draw_list(),
        label,
        at,
        TAG_TEXT_SIZE_PIXELS,
        faded(TAG_COLOR, alpha),
        (faded(SHADOW_COLOR, alpha), SHADOW_OFFSET_PIXELS),
    );
}

fn draw_arrow(
    ui: &Ui,
    label: &str,
    badge: PartnerBadge,
    at: [f32; 2],
    angle_radians: f32,
    screen: [f32; 2],
    alpha: f32,
) {
    let (sin, cos) = angle_radians.sin_cos();
    // screen y points down, so the arrow's up-pointing angle is mirrored in y
    let forward = [cos, -sin];
    let side = [sin, cos];
    let point = |along: f32, across: f32| {
        [
            at[0] + forward[0] * along + side[0] * across,
            at[1] + forward[1] * along + side[1] * across,
        ]
    };
    ui.get_background_draw_list()
        .add_triangle(
            point(ARROW_HALF_LENGTH_PIXELS, 0.0),
            point(-ARROW_HALF_LENGTH_PIXELS, ARROW_HALF_WIDTH_PIXELS),
            point(-ARROW_HALF_LENGTH_PIXELS, -ARROW_HALF_WIDTH_PIXELS),
            faded(ARROW_COLOR, alpha),
        )
        .filled(true)
        .build();
    let size = hud_assets::text_size(ui, label, TAG_TEXT_SIZE_PIXELS);
    let badge_height = badge_height(badge);
    let label_at = [
        (at[0] - forward[0] * LABEL_GAP_PIXELS - size[0] * 0.5).clamp(0.0, screen[0] - size[0]),
        (at[1] - forward[1] * LABEL_GAP_PIXELS - size[1] * 0.5)
            .clamp(0.0, screen[1] - size[1] - badge_height),
    ];
    draw_text(ui, label, label_at, alpha);
    draw_badge(
        ui,
        badge,
        label_at[0] + size[0] * 0.5,
        label_at[1] + size[1] + BADGE_GAP_PIXELS,
        alpha,
    );
}

fn faded(color: [f32; 4], alpha: f32) -> [f32; 4] {
    [color[0], color[1], color[2], color[3] * alpha]
}
