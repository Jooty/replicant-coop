use core::ffi::c_void;

use replicant_game::GameMemory;
use replicant_game::minimap::{
    AREA_BOUNDS_TABLE_RVA, AREA_OFFSET_TABLE_X_RVA, AREA_OFFSET_TABLE_Y_RVA,
    AREA_TABLE_STRIDE_BYTES, DIGITAL_STYLE_WIDGET_RVA, FRIEND_ICON_TYPE, HUD_AREA_OFFSET,
    HUD_DIGITAL_STYLE_CONTAINER_OFFSET, HUD_MAP_IMG_CONTAINER_OFFSET, HUD_MINIMUM_SIZE_BYTES,
    HUD_NORMAL_STYLE_CONTAINER_OFFSET, HUD_SHOWN_FLAG_OFFSET, MAP_IMG_HEIGHT_RVA,
    MAP_IMG_WIDGET_RVA, MAP_IMG_WIDTH_RVA, MINIMAP_AREA_COUNT, MINIMAP_ENABLED_BYTE_RVA,
    MINIMAP_PLACE_ICON_RVA, MapGeometry, NORMAL_STYLE_WIDGET_RVA, VIEWPORT_HEIGHT_RVA,
    VIEWPORT_WIDTH_RVA, WORLD_TO_MAP_IGNORED_ARGUMENT_RVA, WORLD_TO_MAP_RVA, minimap_is_shown,
    partner_icon_placement,
};

use crate::memory::InProcessMemory;
use crate::minimap::hook::{HOOKED_BUILD, note_minimap_shown};
use crate::minimap::partner_icon_index;
use crate::net;

const PARTNER_STATE_TIMEOUT_MS: u128 = 3000;
const WIDGET_READABLE_BYTES: usize = 0x90;
const VIEWPORT_HALVING: f32 = 0.5;
const PARTNER_ICON_ANGLE_RADIANS: f32 = 0.0;
const BOUNDS_FLOAT_COUNT: usize = 4;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub(super) enum Skip {
    NoBuild,
    HudMissing,
    MinimapHidden,
    NoPartner,
    WidgetsNotBuilt,
}

pub(super) struct Failure(pub(super) String);

pub(super) struct Placed {
    pub(super) area: u32,
    pub(super) bounds: [f32; BOUNDS_FLOAT_COUNT],
    pub(super) partner_map: [f32; 2],
    pub(super) player_map: [f32; 2],
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) clamped: bool,
    pub(super) index: i32,
}

pub(super) enum Outcome {
    Skipped(Skip),
    Placed(Placed),
}

#[repr(C, align(16))]
struct WorldPosition([f32; 4]);

type GetWidget = unsafe extern "system" fn(u64) -> *mut c_void;
type GetFloat = unsafe extern "system" fn(*mut c_void) -> f32;
type WorldToMap = unsafe extern "system" fn(u64, *mut f32, *const f32, u32) -> *mut f32;
type PlaceIcon = unsafe extern "system" fn(*mut c_void, u8, *const f32, f32, i32);

struct Widgets {
    normal: *mut c_void,
    digital: *mut c_void,
    map_img: *mut c_void,
}

pub(super) fn place_partner_icon(hud: *mut c_void) -> Result<Outcome, Failure> {
    let Some(build) = HOOKED_BUILD.get() else {
        return Ok(Outcome::Skipped(Skip::NoBuild));
    };
    if hud.is_null() {
        return Ok(Outcome::Skipped(Skip::HudMissing));
    }
    let memory = InProcessMemory::new(build);
    let base = memory.module_base();
    let hud_address = hud as usize as u64;
    let mut hud_bytes = [0_u8; 1];
    memory
        .read(hud_address + HUD_MINIMUM_SIZE_BYTES, &mut hud_bytes)
        .map_err(|_| Failure(format!("the HUD object at {hud:p} is not readable")))?;
    let area = memory
        .read_i32(hud_address + HUD_AREA_OFFSET)
        .map_err(|_| Failure("cannot read the HUD area".to_string()))? as u32;
    let shown = memory
        .read_u8(hud_address + HUD_SHOWN_FLAG_OFFSET)
        .unwrap_or(0);
    let enabled = memory
        .read_u8(base + u64::from(MINIMAP_ENABLED_BYTE_RVA))
        .unwrap_or(0);
    let visible = minimap_is_shown(shown, enabled, area, MINIMAP_AREA_COUNT);
    note_minimap_shown(visible);
    if !visible {
        return Ok(Outcome::Skipped(Skip::MinimapHidden));
    }
    let Some((partner, player)) = partner_and_player_positions() else {
        return Ok(Outcome::Skipped(Skip::NoPartner));
    };
    let Some(widgets) = find_widgets(&memory, base, hud_address)? else {
        return Ok(Outcome::Skipped(Skip::WidgetsNotBuilt));
    };
    let geometry = read_geometry(&memory, base, &widgets, area)?;
    let player_map = world_to_map(base, player, area);
    let partner_map = world_to_map(base, partner, area);
    let placement =
        partner_icon_placement(player_map, partner_map, &geometry).ok_or_else(|| {
            Failure(format!(
                "unusable map numbers {geometry:?} {player_map:?} {partner_map:?}"
            ))
        })?;
    let index = partner_icon_index();
    let position = [placement.x, placement.y];
    let place_address = base + u64::from(MINIMAP_PLACE_ICON_RVA);
    for widget in [widgets.normal, widgets.digital] {
        #[expect(unsafe_code, reason = "calls the game's minimap icon placement")]
        // SAFETY: the address is the verified build's icon placement function (widget, icon
        // type, position pointer, view angle, icon index); the widget was validated readable
        // and non-null above; this runs on the game thread right after the HUD tick.
        unsafe {
            core::mem::transmute::<usize, PlaceIcon>(place_address as usize)(
                widget,
                FRIEND_ICON_TYPE,
                position.as_ptr(),
                PARTNER_ICON_ANGLE_RADIANS,
                index,
            );
        }
    }
    Ok(Outcome::Placed(Placed {
        area,
        bounds: read_bounds(&memory, base, area),
        partner_map,
        player_map,
        x: placement.x,
        y: placement.y,
        clamped: placement.clamped_to_edge,
        index,
    }))
}

fn partner_and_player_positions() -> Option<([f32; 3], [f32; 3])> {
    let view = net::handle()?.view();
    let remote = view.partner_state()?;
    let local = view.local.as_ref()?;
    let same_phase = remote.state.phase == local.phase;
    let fresh = remote.age().as_millis() <= PARTNER_STATE_TIMEOUT_MS;
    (same_phase && fresh).then_some((remote.state.position, local.position))
}

fn find_widgets(memory: &InProcessMemory, base: u64, hud: u64) -> Result<Option<Widgets>, Failure> {
    let get = |rva: u32, container_offset: u64| -> *mut c_void {
        #[expect(unsafe_code, reason = "calls a game widget lookup")]
        // SAFETY: the address is a verified-build lookup taking a widget container address,
        // which lies inside the readable HUD object.
        unsafe {
            core::mem::transmute::<usize, GetWidget>((base + u64::from(rva)) as usize)(
                hud + container_offset,
            )
        }
    };
    let widgets = Widgets {
        normal: get(NORMAL_STYLE_WIDGET_RVA, HUD_NORMAL_STYLE_CONTAINER_OFFSET),
        digital: get(DIGITAL_STYLE_WIDGET_RVA, HUD_DIGITAL_STYLE_CONTAINER_OFFSET),
        map_img: get(MAP_IMG_WIDGET_RVA, HUD_MAP_IMG_CONTAINER_OFFSET),
    };
    for (name, widget) in [
        ("normal", widgets.normal),
        ("digital", widgets.digital),
        ("map image", widgets.map_img),
    ] {
        if widget.is_null() {
            return Ok(None);
        }
        let mut probe = [0_u8; WIDGET_READABLE_BYTES];
        memory
            .read(widget as usize as u64, &mut probe)
            .map_err(|_| {
                Failure(format!(
                    "the {name} minimap widget at {widget:p} is not readable"
                ))
            })?;
    }
    Ok(Some(widgets))
}

fn read_geometry(
    memory: &InProcessMemory,
    base: u64,
    widgets: &Widgets,
    area: u32,
) -> Result<MapGeometry, Failure> {
    let call = |rva: u32, widget: *mut c_void| -> f32 {
        #[expect(unsafe_code, reason = "calls a game widget size getter")]
        // SAFETY: the address is a verified-build getter taking a widget and returning a float;
        // the widget was validated readable above.
        unsafe {
            core::mem::transmute::<usize, GetFloat>((base + u64::from(rva)) as usize)(widget)
        }
    };
    let row = u64::from(area) * AREA_TABLE_STRIDE_BYTES;
    let offset = |rva: u32| memory.read_f32(base + u64::from(rva) + row);
    Ok(MapGeometry {
        map_width: call(MAP_IMG_WIDTH_RVA, widgets.map_img),
        map_height: call(MAP_IMG_HEIGHT_RVA, widgets.map_img),
        half_viewport_width: call(VIEWPORT_WIDTH_RVA, widgets.normal) * VIEWPORT_HALVING,
        half_viewport_height: call(VIEWPORT_HEIGHT_RVA, widgets.normal) * VIEWPORT_HALVING,
        offset_x: offset(AREA_OFFSET_TABLE_X_RVA)
            .map_err(|_| Failure("cannot read the area offset table".to_string()))?,
        offset_y: offset(AREA_OFFSET_TABLE_Y_RVA)
            .map_err(|_| Failure("cannot read the area offset table".to_string()))?,
    })
}

fn world_to_map(base: u64, position: [f32; 3], area: u32) -> [f32; 2] {
    let input = WorldPosition([position[0], position[1], position[2], 0.0]);
    let mut output = [-1.0_f32; 2];
    #[expect(unsafe_code, reason = "calls the game's world to map conversion")]
    // SAFETY: the address is the verified build's pure conversion (ignored argument, output
    // pair, input position of four floats, area below the area count); both pointers are live
    // locals and the input is 16-byte aligned.
    unsafe {
        core::mem::transmute::<usize, WorldToMap>((base + u64::from(WORLD_TO_MAP_RVA)) as usize)(
            base + u64::from(WORLD_TO_MAP_IGNORED_ARGUMENT_RVA),
            output.as_mut_ptr(),
            input.0.as_ptr(),
            area,
        );
    }
    output
}

fn read_bounds(memory: &InProcessMemory, base: u64, area: u32) -> [f32; BOUNDS_FLOAT_COUNT] {
    let row = base + u64::from(AREA_BOUNDS_TABLE_RVA) + u64::from(area) * AREA_TABLE_STRIDE_BYTES;
    let mut bounds = [f32::NAN; BOUNDS_FLOAT_COUNT];
    for (index, slot) in bounds.iter_mut().enumerate() {
        if let Ok(value) = memory.read_f32(row + 4 * index as u64) {
            *slot = value;
        }
    }
    bounds
}
