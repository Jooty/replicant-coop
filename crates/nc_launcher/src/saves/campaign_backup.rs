use std::io;
use std::path::Path;

use replicant_game::save_files::COOP_GAMEDATA_FILE_NAME;

use crate::saves::campaigns::campaigns_path_beside;
use crate::saves::copy_dir_recursive;
use crate::saves::seats::seats_dir_beside;

pub(super) fn copy_companions_into(coop_file: &Path, destination: &Path) -> io::Result<()> {
    let record = campaigns_path_beside(coop_file);
    if record.is_file() {
        std::fs::copy(&record, destination.join(file_name_of(&record)))?;
    }
    let seats = seats_dir_beside(coop_file);
    if seats.is_dir() {
        copy_dir_recursive(&seats, &destination.join(file_name_of(&seats)))?;
    }
    Ok(())
}

pub(super) fn restore_companions_from(backup_dir: &Path, coop_file: &Path) -> io::Result<()> {
    let record = campaigns_path_beside(coop_file);
    let backed_up_record = backup_dir.join(file_name_of(&record));
    if backed_up_record.is_file() {
        std::fs::copy(&backed_up_record, &record)?;
    } else if record.is_file() {
        std::fs::remove_file(&record)?;
    }
    let seats = seats_dir_beside(coop_file);
    let backed_up_seats = backup_dir.join(file_name_of(&seats));
    if seats.is_dir() {
        std::fs::remove_dir_all(&seats)?;
    }
    if backed_up_seats.is_dir() {
        copy_dir_recursive(&backed_up_seats, &seats)?;
    }
    Ok(())
}

fn file_name_of(path: &Path) -> &std::ffi::OsStr {
    path.file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new(COOP_GAMEDATA_FILE_NAME))
}
