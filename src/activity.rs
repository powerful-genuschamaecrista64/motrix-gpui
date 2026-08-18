use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Daily downloaded-bytes log, persisted as JSON in the config dir.
/// Powers the Dashboard activity heatmap.
pub struct ActivityLog {
    /// day number (days since Unix epoch) -> bytes downloaded that day
    days: HashMap<i64, u64>,
    dirty_ticks: u32,
}

fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("motrix-gpui")
        .join("activity.json")
}

pub fn today() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    // Local-ish day bucket; good enough for a heatmap.
    (secs + local_utc_offset_secs()) / 86400
}

pub fn local_utc_offset_secs() -> i64 {
    // `date +%z` style offset without pulling in chrono: read TZ via libc's
    // localtime would need unsafe; a fixed cache from the `date` tool is
    // overkill. Use the offset embedded in the `TZ`-aware formatting of
    // std::process once at startup.
    use std::sync::OnceLock;
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(|| {
        std::process::Command::new("date")
            .arg("+%z")
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.len() >= 5 {
                    let sign = if s.starts_with('-') { -1 } else { 1 };
                    let hours: i64 = s[1..3].parse().ok()?;
                    let mins: i64 = s[3..5].parse().ok()?;
                    Some(sign * (hours * 3600 + mins * 60))
                } else {
                    None
                }
            })
            .unwrap_or(0)
    })
}

/// (year, month, day) from days-since-epoch — Howard Hinnant's civil_from_days.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl ActivityLog {
    pub fn load() -> Self {
        let days = std::fs::read_to_string(path())
            .ok()
            .and_then(|text| serde_json::from_str::<HashMap<String, u64>>(&text).ok())
            .map(|m| {
                m.into_iter()
                    .filter_map(|(k, v)| k.parse::<i64>().ok().map(|k| (k, v)))
                    .collect()
            })
            .unwrap_or_default();
        ActivityLog {
            days,
            dirty_ticks: 0,
        }
    }

    /// Record bytes downloaded since the last poll tick.
    pub fn record(&mut self, bytes: u64) {
        if bytes > 0 {
            *self.days.entry(today()).or_insert(0) += bytes;
            self.dirty_ticks += 1;
        } else {
            self.dirty_ticks = self.dirty_ticks.saturating_add(u32::from(self.dirty_ticks > 0));
        }
        if self.dirty_ticks >= 30 {
            self.save();
            self.dirty_ticks = 0;
        }
    }

    pub fn save(&self) {
        let map: HashMap<String, u64> = self
            .days
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        if let Ok(text) = serde_json::to_string(&map) {
            let p = path();
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(p, text);
        }
    }

    pub fn bytes_on(&self, day: i64) -> u64 {
        self.days.get(&day).copied().unwrap_or(0)
    }

    pub fn today_bytes(&self) -> u64 {
        self.bytes_on(today())
    }

    pub fn total_bytes(&self) -> u64 {
        self.days.values().sum()
    }
}
