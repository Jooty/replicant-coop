use std::path::Path;

use nc_launcher::install::{
    GAME_NEVER_APPEARED_SECONDS, GAME_WATCH_SETTLE_SECONDS, GameWatch, GameWatchStep, InstallError,
    InstallOutcome, Ledger, ModStatus, ci_artifact_name, cmdline_runs_exe, end_process_group,
    install_dll, mod_mismatch, mod_status, newest_cached_ci_dll, pids_from_tasklist_csv,
    sha256_file, uninstall_dll,
};

use nc_launcher_core::{Character, GameProfile, GameSupport, REPLICANT};

use crate::scratch::fresh_dir;

fn setup(name: &str) -> (std::path::PathBuf, std::path::PathBuf, Ledger) {
    let base = fresh_dir(name);
    let game = base.join("game");
    std::fs::create_dir_all(&game).unwrap();
    let ledger = Ledger::new(&base.join("cache/installed.sha256"));
    (base, game, ledger)
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn sha256_matches_the_known_digest_of_abc() {
    let base = fresh_dir("sha_abc");
    write(&base.join("f"), b"abc");
    assert_eq!(
        sha256_file(&base.join("f")).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn install_copies_the_dll_once_and_a_repeat_changes_nothing() {
    let (base, game, ledger) = setup("install_twice");
    let source = base.join("replicant_hook.dll");
    write(&source, b"build one");
    assert_eq!(
        install_dll(&REPLICANT, &game, &source, &ledger).unwrap(),
        InstallOutcome::Installed
    );
    assert_eq!(
        install_dll(&REPLICANT, &game, &source, &ledger).unwrap(),
        InstallOutcome::AlreadyCurrent
    );
    assert_eq!(
        std::fs::read(game.join(REPLICANT.loader_dll_name)).unwrap(),
        b"build one"
    );
    assert_eq!(
        mod_status(&REPLICANT, &game, Some(&source), &ledger),
        ModStatus::Current
    );
}

#[test]
fn a_newer_build_replaces_an_older_one_we_installed() {
    let (base, game, ledger) = setup("install_update");
    let source = base.join("replicant_hook.dll");
    write(&source, b"build one");
    install_dll(&REPLICANT, &game, &source, &ledger).unwrap();
    write(&source, b"build two");
    assert_eq!(
        mod_status(&REPLICANT, &game, Some(&source), &ledger),
        ModStatus::Outdated
    );
    assert_eq!(
        install_dll(&REPLICANT, &game, &source, &ledger).unwrap(),
        InstallOutcome::Updated
    );
    assert_eq!(
        std::fs::read(game.join(REPLICANT.loader_dll_name)).unwrap(),
        b"build two"
    );
}

#[test]
fn a_foreign_dll_is_never_overwritten_or_removed() {
    let (base, game, ledger) = setup("install_foreign");
    let source = base.join("replicant_hook.dll");
    write(&source, b"ours");
    write(&game.join(REPLICANT.loader_dll_name), b"someone else's");
    assert!(matches!(
        install_dll(&REPLICANT, &game, &source, &ledger),
        Err(InstallError::ForeignFileInTheWay)
    ));
    assert!(matches!(
        uninstall_dll(&REPLICANT, &game, &ledger),
        Err(InstallError::ForeignFileInTheWay)
    ));
    assert_eq!(
        std::fs::read(game.join(REPLICANT.loader_dll_name)).unwrap(),
        b"someone else's"
    );
    assert_eq!(
        mod_status(&REPLICANT, &game, Some(&source), &ledger),
        ModStatus::Foreign
    );
}

#[test]
fn uninstall_removes_only_our_dll_and_leaves_other_game_files() {
    let (base, game, ledger) = setup("uninstall");
    let source = base.join("replicant_hook.dll");
    write(&source, b"ours");
    write(&game.join("other.dll"), b"keep me");
    install_dll(&REPLICANT, &game, &source, &ledger).unwrap();
    assert!(uninstall_dll(&REPLICANT, &game, &ledger).unwrap());
    assert!(!game.join(REPLICANT.loader_dll_name).exists());
    assert_eq!(std::fs::read(game.join("other.dll")).unwrap(), b"keep me");
    assert!(!uninstall_dll(&REPLICANT, &game, &ledger).unwrap());
    assert_eq!(
        mod_status(&REPLICANT, &game, Some(&source), &ledger),
        ModStatus::NotInstalled
    );
}

#[test]
fn newest_cached_ci_build_is_the_highest_run_id_that_has_a_dll() {
    let base = fresh_dir("cached_ci");
    for run in ["99", "1000", "5000"] {
        std::fs::create_dir_all(base.join(run)).unwrap();
    }
    write(&base.join("99/replicant_hook.dll"), b"a");
    write(&base.join("1000/replicant_hook.dll"), b"b");
    assert_eq!(
        newest_cached_ci_dll(&REPLICANT, &base),
        Some(base.join("1000/replicant_hook.dll"))
    );
}

#[test]
fn game_process_is_recognised_by_its_exe_name_in_the_command_line() {
    let wine = b"Z:\\mnt\\Game\\NieR Replicant ver.1.22474487139.exe\0-arg\0";
    let native = b"/opt/NieR Replicant ver.1.22474487139.exe\0";
    let other = b"/usr/bin/some-other-NieR Replicant ver.1.22474487139.exe\0";
    assert!(cmdline_runs_exe(wine, REPLICANT.exe_names));
    assert!(cmdline_runs_exe(native, REPLICANT.exe_names));
    assert!(!cmdline_runs_exe(other, REPLICANT.exe_names));
    assert!(!cmdline_runs_exe(b"steam\0", REPLICANT.exe_names));
}

#[test]
fn the_more_recently_written_dll_wins() {
    let base = fresh_dir("newer_dll");
    let old = base.join("old.dll");
    let new = base.join("new.dll");
    std::fs::write(&old, b"1").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&new, b"2").unwrap();
    assert_eq!(
        nc_launcher::install::newer_dll(Some(old.clone()), Some(new.clone())),
        Some(new.clone())
    );
    assert_eq!(
        nc_launcher::install::newer_dll(Some(new.clone()), Some(old.clone())),
        Some(new)
    );
    assert_eq!(
        nc_launcher::install::newer_dll(None, Some(old.clone())),
        Some(old)
    );
}

#[test]
fn a_mod_updated_by_another_launcher_is_not_reported_as_different() {
    let (base, game, ledger) = setup("mismatch_stale");
    let old = base.join("old.dll");
    let new = base.join("new.dll");
    write(&old, b"old build");
    write(&new, b"new build");
    install_dll(&REPLICANT, &game, &old, &ledger).unwrap();
    assert_eq!(
        mod_status(&REPLICANT, &game, Some(&new), &ledger),
        ModStatus::Outdated
    );
    assert!(mod_mismatch(&REPLICANT, &game, Some(&new)).is_some());
    install_dll(&REPLICANT, &game, &new, &ledger).unwrap();
    assert_eq!(
        mod_status(&REPLICANT, &game, Some(&new), &ledger),
        ModStatus::Current
    );
    assert_eq!(mod_mismatch(&REPLICANT, &game, Some(&new)), None);
}

#[test]
fn a_real_difference_names_both_short_hashes() {
    let (base, game, ledger) = setup("mismatch_real");
    let old = base.join("old.dll");
    let new = base.join("new.dll");
    write(&old, b"old build");
    write(&new, b"new build");
    install_dll(&REPLICANT, &game, &old, &ledger).unwrap();
    let message = mod_mismatch(&REPLICANT, &game, Some(&new)).unwrap();
    assert!(message.contains(&sha256_file(&old).unwrap()[..8]));
    assert!(message.contains(&sha256_file(&new).unwrap()[..8]));
    assert!(message.contains(REPLICANT.loader_dll_name));
}

#[test]
fn a_tasklist_csv_line_gives_the_pid_of_the_game() {
    let text =
        "\"NieR Replicant ver.1.22474487139.exe\",\"4242\",\"Console\",\"1\",\"1,000 K\"\r\n";
    assert_eq!(
        pids_from_tasklist_csv(text, REPLICANT.exe_names),
        vec![4242]
    );
    assert!(pids_from_tasklist_csv("INFO: No tasks are running.", REPLICANT.exe_names).is_empty());
}

#[test]
fn a_watch_ignores_games_that_were_running_before_the_launch() {
    let mut watch = GameWatch::new(&[10]);
    assert_eq!(watch.step(&[10], 0.0), GameWatchStep::Waiting);
    assert_eq!(watch.step(&[10], 4.0), GameWatchStep::Waiting);
}

#[test]
fn a_watch_reports_running_for_a_new_pid_then_exited_after_two_empty_polls() {
    let mut watch = GameWatch::new(&[10]);
    assert_eq!(watch.step(&[10, 20], 0.0), GameWatchStep::Running);
    assert_eq!(
        watch.step(&[10, 20], GAME_WATCH_SETTLE_SECONDS + 1.0),
        GameWatchStep::Running
    );
    assert_eq!(
        watch.step(&[10], GAME_WATCH_SETTLE_SECONDS + 3.0),
        GameWatchStep::Running
    );
    assert_eq!(
        watch.step(&[10], GAME_WATCH_SETTLE_SECONDS + 5.0),
        GameWatchStep::Exited
    );
}

#[test]
fn a_watch_does_not_call_an_early_wrapper_exit_the_game_closing() {
    let mut watch = GameWatch::new(&[]);
    assert_eq!(watch.step(&[30], 0.0), GameWatchStep::Running);
    assert_eq!(watch.step(&[], 2.0), GameWatchStep::Running);
    assert_eq!(watch.step(&[31], 4.0), GameWatchStep::Running);
    assert_eq!(watch.step(&[31], 6.0), GameWatchStep::Running);
}

#[test]
fn a_watch_stops_adding_pids_after_the_settle_time() {
    let mut watch = GameWatch::new(&[]);
    watch.step(&[1], 0.0);
    let later = GAME_WATCH_SETTLE_SECONDS + 5.0;
    assert_eq!(watch.step(&[1, 2], later), GameWatchStep::Running);
    assert_eq!(watch.step(&[2], later + 2.0), GameWatchStep::Running);
    assert_eq!(watch.step(&[2], later + 4.0), GameWatchStep::Exited);
}

#[test]
fn a_watch_gives_up_when_no_game_ever_appears() {
    let mut watch = GameWatch::new(&[]);
    assert_eq!(watch.step(&[], 0.0), GameWatchStep::Waiting);
    assert_eq!(
        watch.step(&[], GAME_NEVER_APPEARED_SECONDS),
        GameWatchStep::NeverAppeared
    );
}

#[test]
fn a_watch_lists_only_its_own_processes_that_are_still_running() {
    let mut watch = GameWatch::new(&[10]);
    watch.step(&[10, 20, 30], 0.0);
    assert_eq!(watch.pids_running(&[10, 20]), vec![20]);
    assert!(watch.pids_running(&[10]).is_empty());
}

const MADE_UP_CHARACTERS: [Character; 1] = [Character {
    id: "only",
    label: "Only",
}];

const MADE_UP_GAME: GameProfile = GameProfile {
    game_id: "made-up",
    display_name: "Made Up",
    steam_app_id: "424242",
    install_folder_name: "MadeUp",
    exe_names: &["MadeUp.exe", "MadeUpCompat.exe"],
    supported_exe_builds: &[],
    loader_dll_name: "loader.dll",
    hook_dll_name: "made_up_hook.dll",
    link_file_name: "made-up.toml",
    characters: &MADE_UP_CHARACTERS,
    characters_shareable: false,
    character_art: &[],
    modes: &[nc_launcher_core::CO_OP_CAMPAIGN],
    support: GameSupport {
        install: true,
        start: true,
        saves: false,
    },
};

#[test]
fn each_profile_installs_under_its_own_loader_name() {
    let (base, game, ledger) = setup("install_per_profile");
    let source = base.join("made_up_hook.dll");
    write(&source, b"build");
    install_dll(&MADE_UP_GAME, &game, &source, &ledger).unwrap();
    assert!(game.join("loader.dll").is_file());
    assert!(!game.join(REPLICANT.loader_dll_name).exists());
    install_dll(&REPLICANT, &game, &source, &ledger).unwrap();
    assert!(game.join(REPLICANT.loader_dll_name).is_file());
}

#[test]
fn installing_next_to_a_two_exe_game_leaves_both_exes_alone() {
    let (base, game, ledger) = setup("install_two_exes");
    write(&game.join("MadeUp.exe"), b"first exe");
    write(&game.join("MadeUpCompat.exe"), b"second exe");
    let source = base.join("made_up_hook.dll");
    write(&source, b"build");
    install_dll(&MADE_UP_GAME, &game, &source, &ledger).unwrap();
    assert_eq!(
        mod_status(&MADE_UP_GAME, &game, Some(&source), &ledger),
        ModStatus::Current
    );
    assert!(uninstall_dll(&MADE_UP_GAME, &game, &ledger).unwrap());
    assert_eq!(
        std::fs::read(game.join("MadeUp.exe")).unwrap(),
        b"first exe"
    );
    assert_eq!(
        std::fs::read(game.join("MadeUpCompat.exe")).unwrap(),
        b"second exe"
    );
    assert!(!game.join("loader.dll").exists());
}

#[test]
fn a_loader_name_that_is_a_game_exe_is_refused_and_nothing_is_touched() {
    let exe_as_loader = GameProfile {
        loader_dll_name: "MadeUp.exe",
        ..MADE_UP_GAME
    };
    let (base, game, ledger) = setup("loader_is_exe");
    write(&game.join("MadeUp.exe"), b"the game");
    let source = base.join("made_up_hook.dll");
    write(&source, b"build");
    assert!(matches!(
        install_dll(&exe_as_loader, &game, &source, &ledger),
        Err(InstallError::LoaderIsAGameFile)
    ));
    assert!(matches!(
        uninstall_dll(&exe_as_loader, &game, &ledger),
        Err(InstallError::LoaderIsAGameFile)
    ));
    assert_eq!(std::fs::read(game.join("MadeUp.exe")).unwrap(), b"the game");
}

#[test]
fn a_foreign_loader_file_is_never_overwritten_or_deleted() {
    let (base, game, ledger) = setup("foreign_loader");
    write(&game.join("loader.dll"), b"someone else's");
    let source = base.join("made_up_hook.dll");
    write(&source, b"build");
    assert!(matches!(
        install_dll(&MADE_UP_GAME, &game, &source, &ledger),
        Err(InstallError::ForeignFileInTheWay)
    ));
    assert!(matches!(
        uninstall_dll(&MADE_UP_GAME, &game, &ledger),
        Err(InstallError::ForeignFileInTheWay)
    ));
    assert_eq!(
        std::fs::read(game.join("loader.dll")).unwrap(),
        b"someone else's"
    );
}

#[test]
fn a_game_with_two_exes_is_recognised_by_either_exe_in_the_command_line() {
    assert!(cmdline_runs_exe(
        b"/opt/MadeUp.exe\0",
        MADE_UP_GAME.exe_names
    ));
    assert!(cmdline_runs_exe(
        b"Z:\\g\\MadeUpCompat.exe\0",
        MADE_UP_GAME.exe_names
    ));
    assert!(!cmdline_runs_exe(
        b"/opt/Other.exe\0",
        MADE_UP_GAME.exe_names
    ));
    let csv = "\"MadeUpCompat.exe\",\"77\",\"Console\",\"1\",\"1 K\"\r\n";
    assert_eq!(
        pids_from_tasklist_csv(csv, MADE_UP_GAME.exe_names),
        vec![77]
    );
}

#[test]
fn the_ci_artifact_is_named_after_the_game() {
    assert_eq!(ci_artifact_name(&REPLICANT), "replicant-hook-dll");
    let other = GameProfile {
        game_id: "alpha",
        ..REPLICANT
    };
    assert_eq!(ci_artifact_name(&other), "alpha-hook-dll");
}

#[cfg(unix)]
#[test]
fn ending_a_process_group_ends_the_leader_and_its_children() {
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    let mut leader = Command::new("sh");
    leader.args(["-c", "sleep 60 & wait"]).process_group(0);
    let mut child = leader.spawn().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    end_process_group(child.id());
    let status = child.wait().unwrap();
    assert!(!status.success());
    assert_eq!(status.code(), None);
}
