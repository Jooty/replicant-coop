use hudhook::imgui::Ui;

use crate::dialogue::shown_choice_tags;
use crate::net;
use crate::overlay::hud_assets;

const TAG_COLOR: [f32; 4] = [1.0, 0.92, 0.55, 1.0];
const SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.85];
const SHADOW_OFFSET_PIXELS: f32 = 1.0;
const TAG_TEXT_SIZE_PIXELS: f32 = 20.0;
const TAG_CENTER_FRACTION_OF_SCREEN_WIDTH: f32 = 0.5;
const TAG_TOP_FRACTION_OF_SCREEN_HEIGHT: f32 = 0.3;
const PARTNER_NAME_WHEN_UNKNOWN: &str = "your partner";

/// Names the option the other player is on at an answer prompt: on the host's screen the guest's
/// marker, on the guest's screen its own. A fixed place, since the list's place is not read.
pub(super) fn draw_choice_tag(ui: &Ui) {
    let Some(tags) = shown_choice_tags() else {
        return;
    };
    let text = if let Some(option) = tags.own_marker {
        format!("you: option {} (the host decides)", u16::from(option) + 1)
    } else if let Some(option) = tags.partner_option {
        let name = net::handle()
            .and_then(|net| net.view().partner_name().map(str::to_owned))
            .unwrap_or_else(|| PARTNER_NAME_WHEN_UNKNOWN.to_owned());
        format!("{name}: option {}", u16::from(option) + 1)
    } else {
        return;
    };
    let [width, height] = ui.io().display_size;
    let size = hud_assets::text_size(ui, &text, TAG_TEXT_SIZE_PIXELS);
    hud_assets::draw_text(
        &ui.get_background_draw_list(),
        &text,
        [
            width * TAG_CENTER_FRACTION_OF_SCREEN_WIDTH - size[0] * 0.5,
            height * TAG_TOP_FRACTION_OF_SCREEN_HEIGHT,
        ],
        TAG_TEXT_SIZE_PIXELS,
        TAG_COLOR,
        (SHADOW_COLOR, SHADOW_OFFSET_PIXELS),
    );
}
