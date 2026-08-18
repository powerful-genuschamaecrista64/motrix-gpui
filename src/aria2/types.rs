use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    Active,
    Waiting,
    Paused,
    Complete,
    Error,
    Removed,
    Seeding,
}

impl TaskStatus {
    pub fn label(&self) -> &'static str {
        match self {
            TaskStatus::Active => "Downloading",
            TaskStatus::Waiting => "Waiting",
            TaskStatus::Paused => "Paused",
            TaskStatus::Complete => "Completed",
            TaskStatus::Error => "Error",
            TaskStatus::Removed => "Removed",
            TaskStatus::Seeding => "Seeding",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawFile {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub length: Option<String>,
    #[serde(default)]
    pub completed_length: Option<String>,
    #[serde(default)]
    pub selected: Option<String>,
    #[serde(default)]
    pub uris: Vec<RawUri>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawUri {
    #[serde(default)]
    pub uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBittorrentInfo {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBittorrent {
    #[serde(default)]
    pub info: Option<RawBittorrentInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTask {
    pub gid: String,
    pub status: String,
    #[serde(default)]
    pub total_length: Option<String>,
    #[serde(default)]
    pub completed_length: Option<String>,
    #[serde(default)]
    pub upload_length: Option<String>,
    #[serde(default)]
    pub download_speed: Option<String>,
    #[serde(default)]
    pub upload_speed: Option<String>,
    #[serde(default)]
    pub connections: Option<String>,
    #[serde(default)]
    pub num_seeders: Option<String>,
    #[serde(default)]
    pub seeder: Option<String>,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub files: Vec<RawFile>,
    #[serde(default)]
    pub bittorrent: Option<RawBittorrent>,
    #[serde(default)]
    pub info_hash: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
}

/// UI-facing task model, normalized from aria2's string-typed JSON.
#[derive(Debug, Clone)]
pub struct Task {
    pub gid: String,
    pub name: String,
    pub status: TaskStatus,
    pub total_length: u64,
    pub completed_length: u64,
    pub upload_length: u64,
    pub download_speed: u64,
    pub upload_speed: u64,
    pub connections: u32,
    pub num_seeders: u32,
    pub dir: String,
    pub file_path: Option<String>,
    pub file_count: usize,
    pub is_bt: bool,
    pub error_message: Option<String>,
    pub uri: Option<String>,
}

fn p64(v: &Option<String>) -> u64 {
    v.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0)
}

impl Task {
    pub fn from_raw(raw: RawTask) -> Self {
        let is_bt = raw.bittorrent.is_some() || raw.info_hash.is_some();
        let seeding = raw.status == "active"
            && is_bt
            && raw.seeder.as_deref() == Some("true");
        let status = match raw.status.as_str() {
            "active" if seeding => TaskStatus::Seeding,
            "active" => TaskStatus::Active,
            "waiting" => TaskStatus::Waiting,
            "paused" => TaskStatus::Paused,
            "complete" => TaskStatus::Complete,
            "error" => TaskStatus::Error,
            "removed" => TaskStatus::Removed,
            _ => TaskStatus::Waiting,
        };

        let selected_files: Vec<&RawFile> = raw
            .files
            .iter()
            .filter(|f| f.selected.as_deref() != Some("false"))
            .collect();
        let first_file = selected_files
            .first()
            .copied()
            .or_else(|| raw.files.first());

        let bt_name = raw
            .bittorrent
            .as_ref()
            .and_then(|b| b.info.as_ref())
            .and_then(|i| i.name.clone())
            .filter(|n| !n.is_empty());

        let uri = first_file
            .and_then(|f| f.uris.first())
            .map(|u| u.uri.clone())
            .filter(|u| !u.is_empty());

        let name = bt_name
            .or_else(|| {
                first_file.and_then(|f| {
                    std::path::Path::new(&f.path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .filter(|n| !n.is_empty())
                })
            })
            .or_else(|| {
                uri.as_deref().and_then(|u| {
                    u.split('/')
                        .next_back()
                        .map(|s| s.split('?').next().unwrap_or(s).to_string())
                        .filter(|s| !s.is_empty())
                })
            })
            .unwrap_or_else(|| format!("Task {}", raw.gid));

        let file_path = first_file.map(|f| f.path.clone()).filter(|p| !p.is_empty());

        Task {
            gid: raw.gid,
            name,
            status,
            total_length: p64(&raw.total_length),
            completed_length: p64(&raw.completed_length),
            upload_length: p64(&raw.upload_length),
            download_speed: p64(&raw.download_speed),
            upload_speed: p64(&raw.upload_speed),
            connections: p64(&raw.connections) as u32,
            num_seeders: p64(&raw.num_seeders) as u32,
            dir: raw.dir.unwrap_or_default(),
            file_path,
            file_count: selected_files.len().max(1),
            is_bt,
            error_message: raw.error_message.filter(|m| !m.is_empty()),
            uri,
        }
    }

    pub fn progress(&self) -> f32 {
        if self.total_length == 0 {
            0.0
        } else {
            (self.completed_length as f64 / self.total_length as f64) as f32
        }
    }

    /// Remaining seconds, or None when unknown.
    pub fn eta_seconds(&self) -> Option<u64> {
        if self.download_speed == 0 || self.total_length <= self.completed_length {
            return None;
        }
        Some((self.total_length - self.completed_length) / self.download_speed)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawGlobalStat {
    #[serde(default)]
    pub download_speed: Option<String>,
    #[serde(default)]
    pub upload_speed: Option<String>,
    #[serde(default)]
    pub num_active: Option<String>,
    #[serde(default)]
    pub num_waiting: Option<String>,
    #[serde(default)]
    pub num_stopped_total: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GlobalStat {
    pub download_speed: u64,
    pub upload_speed: u64,
    pub num_active: u32,
    pub num_waiting: u32,
    pub num_stopped: u32,
}

impl GlobalStat {
    pub fn from_raw(raw: RawGlobalStat) -> Self {
        GlobalStat {
            download_speed: p64(&raw.download_speed),
            upload_speed: p64(&raw.upload_speed),
            num_active: p64(&raw.num_active) as u32,
            num_waiting: p64(&raw.num_waiting) as u32,
            num_stopped: p64(&raw.num_stopped_total) as u32,
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes == 0 {
        return "0 B".into();
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec))
}

pub fn format_eta(secs: u64) -> String {
    if secs >= 86400 {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    } else if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}
