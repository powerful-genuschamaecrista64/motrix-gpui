use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    pub theme: ThemeMode,
    pub download_dir: String,
    pub max_concurrent_downloads: u32,
    pub split: u32,
    /// bytes/s, 0 = unlimited
    pub max_overall_download_limit: u64,
    /// bytes/s, 0 = unlimited
    pub max_overall_upload_limit: u64,
    pub user_agent: String,
    pub rpc_port: u16,
    pub rpc_secret: String,
    pub bt_trackers: Vec<String>,
    pub resume_all_when_app_launched: bool,
    pub new_task_show_downloading: bool,
    pub keep_window_state: bool,
    pub notify_on_complete: bool,
    pub notify_on_error: bool,
    pub warn_before_quit: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let download_dir = dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string();
        AppConfig {
            theme: ThemeMode::System,
            download_dir,
            max_concurrent_downloads: 5,
            split: 16,
            max_overall_download_limit: 0,
            max_overall_upload_limit: 0,
            user_agent: "Transmission/4.0.5".into(),
            rpc_port: 29101,
            rpc_secret: String::new(),
            bt_trackers: Vec::new(),
            resume_all_when_app_launched: false,
            new_task_show_downloading: true,
            keep_window_state: true,
            notify_on_complete: true,
            notify_on_error: false,
            warn_before_quit: true,
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("motrix-gpui")
        .join("config.json")
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => {
                let mut config = AppConfig::default();
                // First run: generate a random RPC secret.
                config.rpc_secret = generate_secret();
                config.save();
                config
            }
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }
}

fn generate_secret() -> String {
    // Derive pseudo-random bytes without pulling in a rand crate.
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for _ in 0..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0));
        out.push_str(&format!("{:016x}", hasher.finish()));
    }
    out
}
