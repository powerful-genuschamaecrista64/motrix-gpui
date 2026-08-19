use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use anyhow::{anyhow, Result};

use crate::config::AppConfig;

/// Manages a local aria2c process with RPC enabled.
pub struct Aria2Daemon {
    child: Option<Child>,
    pub port: u16,
    pub secret: String,
}

#[cfg(windows)]
const ARIA2C_EXE: &str = "aria2c.exe";
#[cfg(not(windows))]
const ARIA2C_EXE: &str = "aria2c";

/// Per-platform hint shown when aria2c cannot be found.
pub fn install_hint() -> &'static str {
    if cfg!(target_os = "windows") {
        "winget install aria2"
    } else if cfg!(target_os = "macos") {
        "brew install aria2"
    } else {
        "sudo apt install aria2"
    }
}

fn find_aria2c() -> Option<PathBuf> {
    // A binary bundled alongside the app executable (Windows/Linux archives
    // ship aria2c next to motrix; a macOS bundle would use Resources).
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)) {
        let p = dir.join(ARIA2C_EXE);
        if p.is_file() {
            return Some(p);
        }
        if cfg!(target_os = "macos") {
            if let Some(parent) = dir.parent() {
                let p = parent.join("Resources").join(ARIA2C_EXE);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }

    // PATH lookup, without shelling out (GUI apps inherit a minimal PATH,
    // hence the fixed candidates below).
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let p = dir.join(ARIA2C_EXE);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    let candidates: &[&str] = if cfg!(windows) {
        &[
            r"C:\Program Files\aria2\aria2c.exe",
            r"C:\ProgramData\chocolatey\bin\aria2c.exe",
        ]
    } else {
        &[
            "/opt/homebrew/bin/aria2c",
            "/usr/local/bin/aria2c",
            "/usr/bin/aria2c",
        ]
    };
    candidates.iter().map(PathBuf::from).find(|p| p.is_file())
}

fn data_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("motrix-gpui");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

impl Aria2Daemon {
    /// Spawn aria2c. Returns an error if no binary can be found.
    pub fn spawn(config: &AppConfig) -> Result<Self> {
        let bin = find_aria2c()
            .ok_or_else(|| anyhow!("aria2c not found. Install it with `{}`.", install_hint()))?;

        let port = config.rpc_port;
        let secret = config.rpc_secret.clone();
        let session = data_dir().join("aria2.session");
        if !session.exists() {
            let _ = std::fs::write(&session, b"");
        }

        let mut cmd = Command::new(bin);
        cmd.arg("--enable-rpc=true")
            .arg(format!("--rpc-listen-port={port}"))
            .arg("--rpc-allow-origin-all=true")
            .arg("--rpc-listen-all=false")
            .arg(format!("--dir={}", config.download_dir))
            .arg(format!("--save-session={}", session.display()))
            .arg(format!("--input-file={}", session.display()))
            .arg("--save-session-interval=30")
            .arg("--continue=true")
            .arg(format!(
                "--max-concurrent-downloads={}",
                config.max_concurrent_downloads
            ))
            .arg(format!("--split={}", config.split))
            .arg(format!(
                "--max-connection-per-server={}",
                config.split.min(16)
            ))
            .arg("--min-split-size=1M")
            .arg("--allow-overwrite=false")
            .arg("--auto-file-renaming=true")
            .arg("--follow-torrent=true")
            .arg("--bt-save-metadata=true")
            .arg("--enable-dht=true")
            .arg("--enable-peer-exchange=true")
            .arg("--seed-ratio=1.0")
            .arg("--seed-time=60")
            .arg("--file-allocation=none")
            .arg("--quiet=true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        if !secret.is_empty() {
            cmd.arg(format!("--rpc-secret={secret}"));
        }
        if !config.user_agent.is_empty() {
            cmd.arg(format!("--user-agent={}", config.user_agent));
        }
        if config.max_overall_download_limit > 0 {
            cmd.arg(format!(
                "--max-overall-download-limit={}",
                config.max_overall_download_limit
            ));
        }
        if config.max_overall_upload_limit > 0 {
            cmd.arg(format!(
                "--max-overall-upload-limit={}",
                config.max_overall_upload_limit
            ));
        }
        let trackers = if config.bt_trackers.is_empty() {
            crate::assets::builtin_trackers()
        } else {
            config.bt_trackers.clone()
        };
        if !trackers.is_empty() {
            cmd.arg(format!("--bt-tracker={}", trackers.join(",")));
        }

        let child = cmd.spawn()?;
        Ok(Aria2Daemon {
            child: Some(child),
            port,
            secret,
        })
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Aria2Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}
