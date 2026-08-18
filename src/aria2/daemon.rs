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

fn find_aria2c() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin/aria2c",
        "/usr/local/bin/aria2c",
        "/usr/bin/aria2c",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    // Fall back to PATH lookup
    std::process::Command::new("which")
        .arg("aria2c")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(PathBuf::from(s)) }
        })
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
        let bin = find_aria2c().ok_or_else(|| {
            anyhow!("aria2c not found. Install it with `brew install aria2`.")
        })?;

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
