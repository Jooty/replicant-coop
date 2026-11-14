use nc_launcher_core::lobby_permissions;

pub fn host_options_editable(viewer_is_guest: bool) -> bool {
    lobby_permissions(viewer_is_guest).change_options
}
