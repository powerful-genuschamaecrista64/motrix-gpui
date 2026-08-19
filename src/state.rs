use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, AppContext, Context, Entity, Task as GpuiTask};

use crate::activity::ActivityLog;
use crate::aria2::{Aria2Client, Aria2Daemon, GlobalStat, Task, TaskStatus};
use crate::config::AppConfig;

/// Which task collection the Downloads page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFilter {
    All,
    Downloading,
    Waiting,
    Completed,
    Stopped,
}

impl TaskFilter {
    pub fn label(&self) -> &'static str {
        match self {
            TaskFilter::All => "All Downloads",
            TaskFilter::Downloading => "Downloading",
            TaskFilter::Waiting => "Waiting",
            TaskFilter::Completed => "Completed",
            TaskFilter::Stopped => "Stopped",
        }
    }

    pub fn matches(&self, task: &Task) -> bool {
        match self {
            TaskFilter::All => task.status != TaskStatus::Removed,
            TaskFilter::Downloading => {
                matches!(task.status, TaskStatus::Active | TaskStatus::Seeding)
            }
            TaskFilter::Waiting => {
                matches!(task.status, TaskStatus::Waiting | TaskStatus::Paused)
            }
            TaskFilter::Completed => task.status == TaskStatus::Complete,
            TaskFilter::Stopped => {
                matches!(task.status, TaskStatus::Error | TaskStatus::Removed)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus {
    Starting,
    Ready,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Dashboard,
    Downloads,
    Trackers,
    Plugins,
    Notifications,
    Settings,
}

pub struct TaskEvent {
    pub title: &'static str,
    pub name: String,
    pub ok: bool,
    pub time: String,
}

pub struct AppState {
    pub config: AppConfig,
    pub tasks: Vec<Task>,
    pub stat: GlobalStat,
    pub engine_status: EngineStatus,
    pub engine_version: String,
    pub filter: TaskFilter,
    pub search_query: String,
    pub search_open: bool,
    pub selected: HashSet<String>,
    pub route: Route,
    pub client: Arc<Aria2Client>,
    pub activity: ActivityLog,
    pub events: Vec<TaskEvent>,
    /// Last 60 samples (1s apart) of global speeds, for the dashboard charts.
    pub down_history: VecDeque<u64>,
    pub up_history: VecDeque<u64>,
    daemon: Option<Aria2Daemon>,
    _poll: Option<GpuiTask<()>>,
}

impl AppState {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let config = AppConfig::load();
        let daemon = Aria2Daemon::spawn(&config).ok();
        let client = Arc::new(Aria2Client::new(config.rpc_port, config.rpc_secret.clone()));

        if config.resume_all_when_app_launched {
            let client = client.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(2));
                let _ = client.unpause_all();
            });
        }

        cx.new(|cx| {
            let mut state = AppState {
                config,
                tasks: Vec::new(),
                stat: GlobalStat::default(),
                engine_status: if daemon.is_some() {
                    EngineStatus::Starting
                } else {
                    EngineStatus::Offline
                },
                engine_version: String::new(),
                filter: TaskFilter::All,
                search_query: String::new(),
                search_open: false,
                selected: HashSet::new(),
                route: Route::Downloads,
                client,
                activity: ActivityLog::load(),
                events: Vec::new(),
                down_history: VecDeque::from(vec![0; 60]),
                up_history: VecDeque::from(vec![0; 60]),
                daemon,
                _poll: None,
            };
            state.start_polling(cx);
            state
        })
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        let client = self.client.clone();
        let task = cx.spawn(async move |this, cx| {
            loop {
                let client2 = client.clone();
                let result = cx
                    .background_spawn(async move {
                        let tasks = client2.fetch_all_tasks()?;
                        let stat = client2.get_global_stat()?;
                        anyhow::Ok((tasks, stat))
                    })
                    .await;

                let ok = this
                    .update(cx, |state, cx| {
                        match result {
                            Ok((tasks, stat)) => {
                                state.notify_transitions(&tasks);
                                state.activity.record(stat.download_speed);
                                state.down_history.push_back(stat.download_speed);
                                state.up_history.push_back(stat.upload_speed);
                                while state.down_history.len() > 60 {
                                    state.down_history.pop_front();
                                }
                                while state.up_history.len() > 60 {
                                    state.up_history.pop_front();
                                }
                                state.tasks = tasks;
                                state.stat = stat;
                                if state.engine_status != EngineStatus::Ready {
                                    state.engine_status = EngineStatus::Ready;
                                    let c = state.client.clone();
                                    let version = cx.background_spawn(async move {
                                        c.get_version().unwrap_or_default()
                                    });
                                    cx.spawn(async move |this, cx| {
                                        let v = version.await;
                                        this.update(cx, |state, cx| {
                                            state.engine_version = v;
                                            cx.notify();
                                        })
                                        .ok();
                                    })
                                    .detach();
                                }
                            }
                            Err(_) => {
                                if state.engine_status == EngineStatus::Ready {
                                    state.engine_status = EngineStatus::Offline;
                                }
                                // No daemon was ever started (aria2c missing at
                                // launch or the spawn failed) — keep trying, so
                                // installing aria2 works without a restart.
                                if state.daemon.is_none() {
                                    if let Ok(daemon) = Aria2Daemon::spawn(&state.config) {
                                        state.daemon = Some(daemon);
                                        state.engine_status = EngineStatus::Starting;
                                    }
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_ok();
                if !ok {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;
            }
        });
        self._poll = Some(task);
    }

    pub fn filtered_tasks(&self) -> Vec<&Task> {
        let query = self.search_query.trim().to_lowercase();
        self.tasks
            .iter()
            .filter(|t| self.filter.matches(t))
            .filter(|t| query.is_empty() || t.name.to_lowercase().contains(&query))
            .collect()
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        let mut active = 0;
        let mut completed = 0;
        let mut error = 0;
        for t in &self.tasks {
            match t.status {
                TaskStatus::Active | TaskStatus::Seeding => active += 1,
                TaskStatus::Complete => completed += 1,
                TaskStatus::Error => error += 1,
                _ => {}
            }
        }
        (active, completed, error)
    }

    /// Run an RPC call on the background executor; UI refreshes on next poll.
    pub fn run_rpc(
        &self,
        cx: &mut Context<Self>,
        f: impl FnOnce(Arc<Aria2Client>) -> anyhow::Result<()> + Send + 'static,
    ) {
        let client = self.client.clone();
        cx.background_spawn(async move {
            if let Err(err) = f(client) {
                log_rpc_error(&err);
            }
        })
        .detach();
    }

    /// Record task completion/failure events and fire system notifications.
    fn notify_transitions(&mut self, new_tasks: &[Task]) {
        let mut events = Vec::new();
        for task in new_tasks {
            let Some(old) = self.tasks.iter().find(|t| t.gid == task.gid) else {
                continue;
            };
            let was_running = matches!(
                old.status,
                TaskStatus::Active | TaskStatus::Waiting | TaskStatus::Seeding
            );
            if !was_running {
                continue;
            }
            if task.status == TaskStatus::Complete {
                events.push(TaskEvent {
                    title: "Download complete",
                    name: task.name.clone(),
                    ok: true,
                    time: now_hhmm(),
                });
                if self.config.notify_on_complete {
                    system_notification("Download complete", &task.name);
                }
            } else if task.status == TaskStatus::Error {
                events.push(TaskEvent {
                    title: "Download failed",
                    name: task.name.clone(),
                    ok: false,
                    time: now_hhmm(),
                });
                if self.config.notify_on_error {
                    system_notification("Download failed", &task.name);
                }
            }
        }
        for e in events {
            self.events.insert(0, e);
        }
        self.events.truncate(100);
    }

    /// True when we have no aria2 process of our own and RPC is unreachable —
    /// i.e. aria2c is most likely not installed.
    pub fn aria2_missing(&self) -> bool {
        self.daemon.is_none() && self.engine_status != EngineStatus::Ready
    }

    pub fn has_running_tasks(&self) -> bool {
        self.tasks.iter().any(|t| {
            matches!(
                t.status,
                TaskStatus::Active | TaskStatus::Waiting | TaskStatus::Seeding
            )
        })
    }

    pub fn shutdown(&mut self) {
        self.activity.save();
        if let Some(daemon) = &mut self.daemon {
            let _ = self.client.save_session();
            let _ = self.client.shutdown();
            std::thread::sleep(Duration::from_millis(200));
            daemon.stop();
        }
    }
}

fn now_hhmm() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64
        + crate::activity::local_utc_offset_secs();
    format!("{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60)
}

fn log_rpc_error(err: &anyhow::Error) {
    eprintln!("aria2 rpc error: {err:#}");
}

fn system_notification(title: &str, body: &str) {
    crate::notifications::send(title, body);
}
