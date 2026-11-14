use hudhook::imgui::{DrawListMut, TextureId, Ui};
use replicant_game::downed::DownedState;
use replicant_game::party_frame::{
    GaugeKind, GaugeQuads, PartyFrameLayout, PartyStatus, TexturedQuad, gauge_length_ratio,
    gauge_quads, health_fraction, party_frame_layout, party_frame_visible, party_status,
};

use crate::cutscene_skip::local_in_cutscene;
use crate::downed::downed_status;
use crate::frame_tuning::frame_tuning;
use crate::net::{self, NetShared};
use crate::overlay::hud_assets::{GaugeTextures, HudTextures, gauge_textures, textures};
use crate::player_names::local_player_name;

const TEXT_COLOR: [f32; 4] = [0.84, 0.81, 0.71, 1.0];
const STATUS_COLOR: [f32; 4] = [0.93, 0.74, 0.62, 1.0];
const TEXT_SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.7];
const TEXT_SHADOW_OFFSET_PIXELS: f32 = 1.5;
const WHITE: [f32; 4] = [1.0; 4];
const LIFE_FILL_TINT: [f32; 4] = [0.55, 0.86, 0.32, 1.0];
const MAGIC_FILL_TINT: [f32; 4] = [0.36, 0.72, 0.95, 1.0];
const FILL_TINT_DOWNED: [f32; 4] = [0.92, 0.26, 0.2, 1.0];
const FILL_TINT_OTHER_AREA: [f32; 4] = [0.55, 0.78, 0.85, 1.0];
const FILL_TINT_DISCONNECTED: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
const BASE_TINT_DISCONNECTED: [f32; 4] = [1.0, 1.0, 1.0, 0.5];
const PLAIN_FRAME_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.9];
const PLAIN_FRAME_THICKNESS_SPRITE_PIXELS: f32 = 3.0;
const EMPTY_WELL_COLOR: [f32; 4] = [0.03, 0.04, 0.03, 0.85];

pub(super) fn draw_party_frame(ui: &Ui) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    if !party_frame_visible(view.options.show_party_frame, local_in_cutscene()) {
        return;
    }
    let Some(remote) = view.partner_state() else {
        return;
    };
    let status = status_of(&view);
    let activity = remote.state.activity;
    let [width, height] = ui.io().display_size;
    let layout = party_frame_layout(width, height, &frame_tuning());
    let draw_list = ui.get_background_draw_list();
    let font = textures();
    let gauges = gauge_textures();
    let level = remote.state.level;
    if remote.state.max_mp > 0 {
        let magic = gauge_quads(
            GaugeKind::Magic,
            layout.magic_anchor,
            layout.bar_scale,
            gauge_length_ratio(GaugeKind::Magic, level),
            health_fraction(u32::from(remote.state.mp), u32::from(remote.state.max_mp)),
        );
        draw_gauge(
            &draw_list,
            gauges,
            GaugeKind::Magic,
            &magic,
            &layout,
            status,
        );
    }
    // The life bar goes on top so its brush border is not covered by the magic bar's stone edge.
    let life = gauge_quads(
        GaugeKind::Life,
        layout.life_anchor,
        layout.bar_scale,
        gauge_length_ratio(GaugeKind::Life, level),
        health_fraction(remote.state.hp, remote.state.max_hp),
    );
    draw_gauge(&draw_list, gauges, GaugeKind::Life, &life, &layout, status);
    let name = fit_name(font, &remote.name, &layout);
    draw_text(
        &draw_list,
        font,
        &name,
        layout.name_origin,
        layout.name_scale,
        &layout,
        TEXT_COLOR,
    );
    if let Some(own_name) = local_player_name() {
        let own_name = fit_name(font, &own_name, &layout);
        draw_text(
            &draw_list,
            font,
            &own_name,
            layout.own_name_origin,
            layout.name_scale,
            &layout,
            TEXT_COLOR,
        );
    }
    let label = status_label(status).or(activity.label().filter(|_| status == PartyStatus::Up));
    if let Some(label) = label {
        let label_width = text_width(ui, font, label, layout.status_scale);
        let origin = [
            layout.status_right_edge - label_width,
            layout.status_origin_y,
        ];
        draw_text(
            &draw_list,
            font,
            label,
            origin,
            layout.status_scale,
            &layout,
            STATUS_COLOR,
        );
    }
}

fn status_of(view: &NetShared) -> PartyStatus {
    let remote = view.partner_state();
    party_status(
        remote.is_some(),
        remote.map_or(0, |remote| remote.age().as_millis() as u64),
        remote
            .zip(view.local.as_ref())
            .is_some_and(|(remote, local)| remote.state.phase == local.phase),
        downed_status().partner == DownedState::Downed,
    )
}

fn status_label(status: PartyStatus) -> Option<&'static str> {
    match status {
        PartyStatus::Up => None,
        PartyStatus::Downed => Some("downed"),
        PartyStatus::OtherArea => Some("in another area"),
        PartyStatus::Disconnected => Some("disconnected"),
    }
}

fn fill_tint(kind: GaugeKind, status: PartyStatus) -> [f32; 4] {
    match (kind, status) {
        (_, PartyStatus::Disconnected) => FILL_TINT_DISCONNECTED,
        (GaugeKind::Magic, _) => MAGIC_FILL_TINT,
        (GaugeKind::Life, PartyStatus::Up) => LIFE_FILL_TINT,
        (GaugeKind::Life, PartyStatus::Downed) => FILL_TINT_DOWNED,
        (GaugeKind::Life, PartyStatus::OtherArea) => FILL_TINT_OTHER_AREA,
    }
}

fn base_tint(status: PartyStatus) -> [f32; 4] {
    if status == PartyStatus::Disconnected {
        BASE_TINT_DISCONNECTED
    } else {
        WHITE
    }
}

fn text_width(ui: &Ui, font: Option<&HudTextures>, text: &str, text_scale: f32) -> f32 {
    match font {
        Some(font) => font.font.text_width(text, text_scale),
        None => ui.calc_text_size(text)[0],
    }
}

fn fit_name(font: Option<&HudTextures>, name: &str, layout: &PartyFrameLayout) -> String {
    match font {
        Some(font) => font
            .font
            .fit_text(name, layout.name_scale, layout.name_max_width),
        None => name.to_owned(),
    }
}

fn draw_text(
    draw_list: &DrawListMut<'_>,
    font: Option<&HudTextures>,
    text: &str,
    origin: [f32; 2],
    text_scale: f32,
    layout: &PartyFrameLayout,
    color: [f32; 4],
) {
    let shadow = TEXT_SHADOW_OFFSET_PIXELS * layout.scale;
    let Some(font) = font else {
        draw_list.add_text(
            [origin[0] + shadow, origin[1] + shadow],
            TEXT_SHADOW_COLOR,
            text,
        );
        draw_list.add_text(origin, color, text);
        return;
    };
    for (offset, tint) in [(shadow, TEXT_SHADOW_COLOR), (0.0, color)] {
        let quads =
            font.font
                .layout_text(text, [origin[0] + offset, origin[1] + offset], text_scale);
        for quad in quads {
            draw_list
                .add_image(font.font_texture, quad.min, quad.max)
                .uv_min(quad.uv_min)
                .uv_max(quad.uv_max)
                .col(tint)
                .build();
        }
    }
}

fn draw_gauge(
    draw_list: &DrawListMut<'_>,
    gauges: Option<&GaugeTextures>,
    kind: GaugeKind,
    quads: &GaugeQuads,
    layout: &PartyFrameLayout,
    status: PartyStatus,
) {
    let Some(gauges) = gauges else {
        draw_plain_gauge(draw_list, kind, quads, layout, status);
        return;
    };
    let bar = match kind {
        GaugeKind::Life => gauges.life_bar,
        GaugeKind::Magic => gauges.magic_bar,
    };
    if kind.has_brush_frame() {
        for slice in &quads.base {
            draw_quad(draw_list, gauges.life_base, slice, base_tint(status));
        }
    } else {
        draw_empty_well(draw_list, quads);
    }
    if let Some(fill) = &quads.fill {
        draw_quad(draw_list, bar, fill, fill_tint(kind, status));
    }
}

fn draw_empty_well(draw_list: &DrawListMut<'_>, quads: &GaugeQuads) {
    draw_list
        .add_rect(quads.well_min, quads.well_max, EMPTY_WELL_COLOR)
        .filled(true)
        .build();
}

fn draw_quad(draw_list: &DrawListMut<'_>, texture: TextureId, quad: &TexturedQuad, tint: [f32; 4]) {
    draw_list
        .add_image(texture, quad.min, quad.max)
        .uv_min(quad.uv_min)
        .uv_max(quad.uv_max)
        .col(tint)
        .build();
}

fn draw_plain_gauge(
    draw_list: &DrawListMut<'_>,
    kind: GaugeKind,
    quads: &GaugeQuads,
    layout: &PartyFrameLayout,
    status: PartyStatus,
) {
    if kind.has_brush_frame() {
        let frame = PLAIN_FRAME_THICKNESS_SPRITE_PIXELS * layout.bar_scale;
        draw_list
            .add_rect(
                [quads.well_min[0] - frame, quads.well_min[1] - frame],
                [quads.well_max[0] + frame, quads.well_max[1] + frame],
                PLAIN_FRAME_COLOR,
            )
            .filled(true)
            .build();
    }
    draw_empty_well(draw_list, quads);
    if let Some(fill) = &quads.fill {
        draw_list
            .add_rect(fill.min, fill.max, fill_tint(kind, status))
            .filled(true)
            .build();
    }
}
