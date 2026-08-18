use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::types::*;

/// Blocking JSON-RPC client for aria2 over HTTP.
/// Call from a background thread/executor, never on the UI thread.
pub struct Aria2Client {
    endpoint: String,
    secret: String,
    id: AtomicU64,
    agent: ureq::Agent,
}

impl Aria2Client {
    pub fn new(port: u16, secret: impl Into<String>) -> Self {
        Aria2Client {
            endpoint: format!("http://127.0.0.1:{port}/jsonrpc"),
            secret: secret.into(),
            id: AtomicU64::new(1),
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(15))
                .build(),
        }
    }

    fn call(&self, method: &str, mut params: Vec<Value>) -> Result<Value> {
        if !self.secret.is_empty() {
            params.insert(0, json!(format!("token:{}", self.secret)));
        }
        let id = self.id.fetch_add(1, Ordering::Relaxed);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": format!("aria2.{method}"),
            "params": params,
        });
        let resp: Value = self
            .agent
            .post(&self.endpoint)
            .send_json(body)?
            .into_json()?;
        if let Some(err) = resp.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown aria2 error");
            return Err(anyhow!("aria2: {msg}"));
        }
        resp.get("result")
            .cloned()
            .ok_or_else(|| anyhow!("aria2: missing result"))
    }

    fn call_as<T: DeserializeOwned>(&self, method: &str, params: Vec<Value>) -> Result<T> {
        Ok(serde_json::from_value(self.call(method, params)?)?)
    }

    pub fn get_version(&self) -> Result<String> {
        let v = self.call("getVersion", vec![])?;
        Ok(v.get("version")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string())
    }

    pub fn get_global_stat(&self) -> Result<GlobalStat> {
        Ok(GlobalStat::from_raw(self.call_as("getGlobalStat", vec![])?))
    }

    /// Fetch active + waiting + stopped in one pass.
    pub fn fetch_all_tasks(&self) -> Result<Vec<Task>> {
        let keys = json!([
            "gid", "status", "totalLength", "completedLength", "uploadLength",
            "downloadSpeed", "uploadSpeed", "connections", "numSeeders", "seeder",
            "dir", "files", "bittorrent", "infoHash", "errorCode", "errorMessage"
        ]);
        let active: Vec<RawTask> = self.call_as("tellActive", vec![keys.clone()])?;
        let waiting: Vec<RawTask> =
            self.call_as("tellWaiting", vec![json!(0), json!(1000), keys.clone()])?;
        let stopped: Vec<RawTask> =
            self.call_as("tellStopped", vec![json!(0), json!(1000), keys])?;
        Ok(active
            .into_iter()
            .chain(waiting)
            .chain(stopped)
            .map(Task::from_raw)
            .collect())
    }

    pub fn add_uri(&self, uris: Vec<String>, options: Value) -> Result<String> {
        let v = self.call("addUri", vec![json!(uris), options])?;
        Ok(v.as_str().unwrap_or_default().to_string())
    }

    pub fn add_torrent(&self, torrent_base64: String, options: Value) -> Result<String> {
        let v = self.call(
            "addTorrent",
            vec![json!(torrent_base64), json!([]), options],
        )?;
        // addTorrent may return a gid string or an object with followedBy
        Ok(v.as_str().unwrap_or_default().to_string())
    }

    pub fn pause(&self, gid: &str) -> Result<()> {
        // Graceful pause first; aria2 falls back to forcePause for stuck BT tasks.
        if self.call("pause", vec![json!(gid)]).is_err() {
            self.call("forcePause", vec![json!(gid)])?;
        }
        Ok(())
    }

    pub fn unpause(&self, gid: &str) -> Result<()> {
        self.call("unpause", vec![json!(gid)]).map(|_| ())
    }

    pub fn remove(&self, gid: &str) -> Result<()> {
        if self.call("remove", vec![json!(gid)]).is_err() {
            self.call("forceRemove", vec![json!(gid)])?;
        }
        Ok(())
    }

    pub fn remove_download_result(&self, gid: &str) -> Result<()> {
        self.call("removeDownloadResult", vec![json!(gid)])
            .map(|_| ())
    }

    pub fn purge_download_result(&self) -> Result<()> {
        self.call("purgeDownloadResult", vec![]).map(|_| ())
    }

    pub fn pause_all(&self) -> Result<()> {
        if self.call("pauseAll", vec![]).is_err() {
            self.call("forcePauseAll", vec![])?;
        }
        Ok(())
    }

    pub fn unpause_all(&self) -> Result<()> {
        self.call("unpauseAll", vec![]).map(|_| ())
    }

    pub fn change_global_option(&self, options: Value) -> Result<()> {
        self.call("changeGlobalOption", vec![options]).map(|_| ())
    }

    pub fn save_session(&self) -> Result<()> {
        self.call("saveSession", vec![]).map(|_| ())
    }

    pub fn shutdown(&self) -> Result<()> {
        self.call("shutdown", vec![]).map(|_| ())
    }
}
