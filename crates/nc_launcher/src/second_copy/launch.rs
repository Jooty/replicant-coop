use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::data_dir;
use crate::install::end_process_group;
use crate::second_copy::{
    CopyCommand, LOG_ENV, LOGS_FOLDER_NAME, ProtonSetup, SECOND_COPY_LOG_NAME, TOOL_MANIFEST_NAME,
    copy_command, copy_environment, proton_script_from_config_info, required_tool_app_id,
    runtime_entry_point, steam_client_root,
};
use crate::steam::{GameInstall, candidate_steam_roots, hide_console_window};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CopyExit {
    pub code: Option<i32>,
    pub seconds: f64,
}

#[derive(Debug, Default)]
pub struct CopyOutput {
    pub lines: Vec<String>,
    pub exit: Option<CopyExit>,
}

pub struct SecondCopy {
    pid: u32,
    pub command_line: String,
    shared: Arc<Mutex<CopyOutput>>,
}

struct Sink {
    shared: Arc<Mutex<CopyOutput>>,
    file: Mutex<Option<File>>,
}

impl SecondCopy {
    pub fn kill_tree(&self) {
        if cfg!(unix) && self.pid > 1 {
            let _ = Command::new("kill")
                .args(["-TERM", "--", &format!("-{}", self.pid)])
                .status();
        }
    }

    pub fn force_kill_tree(&self) {
        if self.pid > 1 {
            end_process_group(self.pid);
        }
    }

    pub fn take_output(&self) -> CopyOutput {
        let Ok(mut shared) = self.shared.lock() else {
            return CopyOutput::default();
        };
        CopyOutput {
            lines: std::mem::take(&mut shared.lines),
            exit: shared.exit.take(),
        }
    }
}

impl Sink {
    fn record(&self, line: String) {
        if let Ok(mut file) = self.file.lock()
            && let Some(file) = file.as_mut()
        {
            let _ = writeln!(file, "{line}");
        }
        if let Ok(mut shared) = self.shared.lock() {
            shared.lines.push(line);
        }
    }

    fn record_exit(&self, exit: CopyExit) {
        self.record(format!(
            "second copy exited with {} after {:.1} s",
            exit.code.map_or_else(
                || "no exit code (killed by a signal)".to_owned(),
                |code| format!("code {code}")
            ),
            exit.seconds
        ));
        if let Ok(mut shared) = self.shared.lock() {
            shared.exit = Some(exit);
        }
    }
}

pub fn build_command(
    game: &GameInstall,
    link_port: u16,
    name: &str,
    boot_slot: Option<u8>,
    new_game: Option<(u8, &str)>,
    log_path: Option<&str>,
) -> Result<CopyCommand, String> {
    if !game.exe_path().is_file() {
        return Err(format!("game exe not found: {}", game.exe_path().display()));
    }
    let proton = if cfg!(windows) {
        None
    } else {
        Some(proton_setup(game)?)
    };
    let mut envs = copy_environment(link_port, name, boot_slot, new_game);
    if let Some(log_path) = log_path {
        envs.push((LOG_ENV.to_owned(), log_path.to_owned()));
    }
    Ok(copy_command(game, envs, proton.as_ref()))
}

pub fn start_second_copy(command: &CopyCommand) -> Result<SecondCopy, String> {
    let shared = Arc::new(Mutex::new(CopyOutput::default()));
    let sink = Arc::new(Sink {
        shared: shared.clone(),
        file: Mutex::new(open_log_file()),
    });
    let command_line = command.shell_line();
    sink.record(format!("command: {command_line}"));
    sink.record(format!("env names: {}", command.env_names()));
    let mut child = spawn(command).map_err(|error| {
        let text = format!("spawn failed: {error}");
        sink.record(text.clone());
        text
    })?;
    sink.record(format!("spawned pid {}", child.id()));
    if let Some(stdout) = child.stdout.take() {
        read_lines(stdout, "out", sink.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        read_lines(stderr, "err", sink.clone());
    }
    let pid = child.id();
    let started = Instant::now();
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|status| status.code());
        sink.record_exit(CopyExit {
            code,
            seconds: started.elapsed().as_secs_f64(),
        });
    });
    Ok(SecondCopy {
        pid,
        command_line,
        shared,
    })
}

fn proton_setup(game: &GameInstall) -> Result<ProtonSetup, String> {
    let compat = game.proton_compat_dir();
    let config_info = std::fs::read_to_string(compat.join("config_info")).map_err(|error| {
        format!(
            "could not read {}: {error}; start the game once from Steam",
            compat.join("config_info").display()
        )
    })?;
    let script =
        proton_script_from_config_info(&config_info).ok_or("config_info names no Proton build")?;
    if !script.is_file() {
        return Err(format!("Proton script not found: {}", script.display()));
    }
    let candidates = candidate_steam_roots(dirs::home_dir().as_deref(), None);
    let steam_root = steam_client_root(&config_info, &candidates)
        .ok_or("could not find the Steam folder (no steam.sh in the usual places)")?;
    let mut steamapps_dirs = vec![game.steamapps_dir.clone(), steam_root.join("steamapps")];
    steamapps_dirs.dedup();
    let runtime_entry = match script
        .parent()
        .map(|dir| dir.join(TOOL_MANIFEST_NAME))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|manifest| required_tool_app_id(&manifest))
    {
        Some(app_id) => Some(runtime_entry_point(&app_id, &steamapps_dirs).ok_or_else(|| {
            format!("this Proton needs Steam app {app_id} (its container runtime), which is not installed")
        })?),
        None => None,
    };
    Ok(ProtonSetup {
        script,
        steam_root,
        runtime_entry,
    })
}

fn spawn(command: &CopyCommand) -> std::io::Result<std::process::Child> {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .envs(command.envs.iter().map(|(key, value)| (key, value)))
        .current_dir(&command.current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut process, 0);
    hide_console_window(&mut process);
    process.spawn()
}

fn read_lines(stream: impl std::io::Read + Send + 'static, label: &'static str, sink: Arc<Sink>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            sink.record(format!("{label}: {line}"));
        }
    });
}

fn open_log_file() -> Option<File> {
    let dir: PathBuf = data_dir()?.join(LOGS_FOLDER_NAME);
    std::fs::create_dir_all(&dir).ok()?;
    File::create(Path::new(&dir).join(SECOND_COPY_LOG_NAME)).ok()
}
