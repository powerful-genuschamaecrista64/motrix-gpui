use std::collections::HashSet;

use gpui::prelude::FluentBuilder;
use gpui::AppContext as _;
use gpui::{
    div, px, uniform_list, App, ClickEvent, Context, Entity, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    UniformListScrollHandle, Window,
};
use std::cell::Cell;
use std::rc::Rc;

use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, IconName, Sizable, WindowExt};

use crate::aria2::{format_bytes, format_eta, format_speed, Task, TaskStatus};
use crate::state::{AppState, TaskFilter};
use crate::theme::StatusColors;
use crate::ui::status_bar::render_status_bar;

const ROW_HEIGHT: f32 = 48.;

pub struct DownloadsPage {
    state: Entity<AppState>,
    selected: HashSet<String>,
    search_open: bool,
    search_input: Entity<InputState>,
    scroll_handle: UniformListScrollHandle,
}

impl DownloadsPage {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search downloads"));
        cx.subscribe(&search_input, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().to_string();
                this.state.update(cx, |state, cx| {
                    state.search_query = query;
                    cx.notify();
                });
            }
        })
        .detach();

        DownloadsPage {
            state,
            selected: HashSet::new(),
            search_open: false,
            search_input,
            scroll_handle: UniformListScrollHandle::new(),
        }
    }

    fn set_filter(&mut self, filter: TaskFilter, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.filter = filter;
            cx.notify();
        });
        self.selected.clear();
    }

    fn toggle_select(&mut self, gid: &str, additive: bool, cx: &mut Context<Self>) {
        if additive {
            if !self.selected.insert(gid.to_string()) {
                self.selected.remove(gid);
            }
        } else {
            self.selected.clear();
            self.selected.insert(gid.to_string());
        }
        cx.notify();
    }

    fn pause_task(&self, gid: String, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.run_rpc(cx, move |client| client.pause(&gid));
        });
    }

    fn resume_task(&self, gid: String, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.run_rpc(cx, move |client| client.unpause(&gid));
        });
    }

    fn remove_task(&mut self, task_gid: String, status: TaskStatus, cx: &mut Context<Self>) {
        self.selected.remove(&task_gid);
        self.state.update(cx, |state, cx| {
            state.run_rpc(cx, move |client| match status {
                TaskStatus::Active
                | TaskStatus::Waiting
                | TaskStatus::Paused
                | TaskStatus::Seeding => client.remove(&task_gid),
                _ => client.remove_download_result(&task_gid),
            });
        });
    }

    /// Ask before removing, with an option to move the files to the Trash —
    /// same flow as the Electron version.
    fn confirm_remove(&mut self, tasks: Vec<Task>, window: &mut Window, cx: &mut Context<Self>) {
        if tasks.is_empty() {
            return;
        }
        let delete_flag = Rc::new(Cell::new(false));
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let tasks = tasks.clone();
            let flag = delete_flag.clone();
            let flag_toggle = delete_flag.clone();
            let flag_submit = delete_flag.clone();
            let entity = entity.clone();
            let message = if tasks.len() == 1 {
                tasks[0].name.clone()
            } else {
                format!("{} tasks selected", tasks.len())
            };
            dialog
                .rounded(cx.theme().radius_lg)
                .w(px(420.))
                .overlay(true)
                .overlay_closable(true)
                .title(
                    div()
                        .text_size(px(16.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("Remove download?"),
                )
                .child(
                    v_flex()
                        .gap(px(12.))
                        .pt(px(4.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(message),
                        )
                        .child(
                            Checkbox::new("remove-with-files")
                                .checked(flag.get())
                                .label("Also move downloaded files to Trash")
                                .on_click(move |checked: &bool, window, _| {
                                    flag_toggle.set(*checked);
                                    window.refresh();
                                }),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().child(
                            h_flex().justify_end().child(
                                Button::new("cancel").outline().small().label("Cancel"),
                            ),
                        ))
                        .child(DialogAction::new().child(
                            h_flex().justify_end().child(
                                Button::new("remove").danger().small().label("Remove"),
                            ),
                        )),
                )
                .on_ok(move |_, _, cx| {
                    let delete_files = flag_submit.get();
                    entity.update(cx, |this, cx| {
                        for task in tasks.clone() {
                            if delete_files {
                                trash_task_files(&task);
                            }
                            this.remove_task(task.gid, task.status, cx);
                        }
                    });
                    true
                })
        });
    }

    fn restart_task(&self, task: &Task, cx: &mut Context<Self>) {
        let gid = task.gid.clone();
        let uri = task.uri.clone();
        let dir = task.dir.clone();
        self.state.update(cx, |state, cx| {
            state.run_rpc(cx, move |client| {
                let Some(uri) = uri else { return Ok(()) };
                let _ = client.remove_download_result(&gid);
                client.add_uri(vec![uri], serde_json::json!({ "dir": dir }))?;
                Ok(())
            });
        });
    }

    fn open_task_folder(&self, task: &Task) {
        let target = task
            .file_path
            .clone()
            .filter(|p| std::path::Path::new(p).exists());
        if let Some(path) = target {
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn();
        } else if !task.dir.is_empty() {
            let _ = open::that(task.dir.clone());
        }
    }

    fn render_header(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.state.read(cx);
        let filter = st.filter;
        let total = st
            .tasks
            .iter()
            .filter(|t| TaskFilter::All.matches(t))
            .count();
        let shown = st.filtered_tasks().len();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let entity = cx.entity();

        let filter_menu = move |menu: PopupMenu,
                                _w: &mut Window,
                                _cx: &mut Context<PopupMenu>|
              -> PopupMenu {
            let mut menu = menu.min_w(px(200.));
            for f in [
                TaskFilter::All,
                TaskFilter::Downloading,
                TaskFilter::Waiting,
                TaskFilter::Completed,
                TaskFilter::Stopped,
            ] {
                let entity = entity.clone();
                menu = menu.item(
                    PopupMenuItem::new(f.label())
                        .checked(filter == f)
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| this.set_filter(f, cx));
                        }),
                );
            }
            menu
        };

        h_flex()
            .flex_shrink_0()
            .pt(px(32.))
            .px(px(24.))
            .pb(px(16.))
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("filter-title")
                            .ghost()
                            .compact()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .line_height(px(32.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(filter.label().to_string()),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronDown)
                                            .size(px(16.))
                                            .text_color(muted),
                                    ),
                            )
                            .dropdown_menu(filter_menu),
                    )
                    .child(
                        div()
                            .h(px(20.))
                            .px(px(8.))
                            .rounded(px(26.))
                            .border_1()
                            .border_color(border)
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(format!("{shown}/{total}")),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        Button::new("pause-all")
                            .ghost()
                            .small()
                            .icon(IconName::Pause)
                            .rounded(px(999.))
                            .tooltip("Pause all")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |state, cx| {
                                    state.run_rpc(cx, |client| client.pause_all());
                                });
                            })),
                    )
                    .child(
                        Button::new("resume-all")
                            .ghost()
                            .small()
                            .icon(IconName::Play)
                            .rounded(px(999.))
                            .tooltip("Resume all")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |state, cx| {
                                    state.run_rpc(cx, |client| client.unpause_all());
                                });
                            })),
                    )
                    .when(self.search_open, |this| {
                        this.child(
                            div()
                                .w(px(220.))
                                .child(Input::new(&self.search_input).small()),
                        )
                    })
                    .child(
                        Button::new("search-toggle")
                            .ghost()
                            .icon(IconName::Search)
                            .small()
                            .rounded(px(999.))
                            .on_click(cx.listener({
                                let input = self.search_input.clone();
                                move |this, _: &ClickEvent, window, cx| {
                                    this.search_open = !this.search_open;
                                    if this.search_open {
                                        input.update(cx, |input, cx| {
                                            input.focus(window, cx);
                                        });
                                    } else {
                                        input.update(cx, |input, cx| {
                                            input.set_value("", window, cx);
                                        });
                                        this.state.update(cx, |state, cx| {
                                            state.search_query.clear();
                                            cx.notify();
                                        });
                                    }
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }

    fn render_selection_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.selected.is_empty() {
            return None;
        }
        let n = self.selected.len();
        let gids: Vec<String> = self.selected.iter().cloned().collect();
        let tasks: Vec<Task> = {
            let st = self.state.read(cx);
            st.tasks
                .iter()
                .filter(|t| self.selected.contains(&t.gid))
                .cloned()
                .collect()
        };

        let pause_gids = gids.clone();
        let resume_gids = gids.clone();

        Some(
            h_flex()
                .flex_shrink_0()
                .mx(px(24.))
                .mb(px(8.))
                .px(px(12.))
                .py(px(8.))
                .gap(px(8.))
                .rounded(px(10.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted.opacity(0.5))
                .items_center()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{n} selected")),
                )
                .child(div().flex_1())
                .child(
                    Button::new("sel-pause")
                        .outline()
                        .xsmall()
                        .icon(IconName::Pause)
                        .label("Pause")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            for gid in pause_gids.clone() {
                                this.pause_task(gid, cx);
                            }
                        })),
                )
                .child(
                    Button::new("sel-resume")
                        .outline()
                        .xsmall()
                        .icon(IconName::Play)
                        .label("Resume")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            for gid in resume_gids.clone() {
                                this.resume_task(gid, cx);
                            }
                        })),
                )
                .child(
                    Button::new("sel-remove")
                        .outline()
                        .xsmall()
                        .icon(Icon::new(crate::assets::AppIcon::Trash))
                        .label("Remove")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_remove(tasks.clone(), window, cx);
                        })),
                ),
        )
    }

    fn render_column_header(&self, cx: &App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let cell = |w: f32, label: &'static str| {
            div()
                .w(px(w))
                .flex_shrink_0()
                .text_size(px(10.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(muted)
                .child(label.to_uppercase())
        };
        h_flex()
            .flex_shrink_0()
            .h(px(36.))
            .px(px(12.))
            .gap(px(12.))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted.opacity(0.4))
            .child(
                div()
                    .flex_1()
                    .min_w(px(120.))
                    .text_size(px(10.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("NAME"),
            )
            .child(cell(70., "Size"))
            .child(cell(120., "Progress"))
            .child(cell(84., "Status"))
            .child(cell(84., "↓"))
            .child(cell(64., "ETA"))
    }

    fn render_row(&self, task: &Task, cx: &mut Context<Self>) -> impl IntoElement {
        let gid = task.gid.clone();
        let selected = self.selected.contains(&gid);
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let accent = cx.theme().accent;
        let border = cx.theme().border;
        // Clearly visible selection tint (the pale gray of the original is
        // invisible on our card background).
        let sel_bg = {
            let mut c: gpui::Hsla = gpui::rgb(StatusColors::BLUE_500).into();
            c.a = if cx.theme().is_dark() { 0.18 } else { 0.10 };
            c
        };

        let (pill_bg, pill_fg, bar_color, pill_label) = match task.status {
            TaskStatus::Active => (
                StatusColors::BLUE_100,
                StatusColors::BLUE_800,
                StatusColors::BLUE_500,
                "Downloading",
            ),
            TaskStatus::Waiting => (
                StatusColors::BLUE_100,
                StatusColors::BLUE_800,
                StatusColors::BLUE_500,
                "Queued",
            ),
            TaskStatus::Paused => (
                StatusColors::AMBER_100,
                StatusColors::AMBER_700,
                StatusColors::AMBER_500,
                "Paused",
            ),
            TaskStatus::Seeding => (
                StatusColors::GREEN_100,
                StatusColors::GREEN_700,
                StatusColors::GREEN_500,
                "Seeding",
            ),
            TaskStatus::Complete => (
                StatusColors::SLATE_100,
                StatusColors::SLATE_600,
                StatusColors::GREEN_500,
                "Completed",
            ),
            TaskStatus::Error => (
                StatusColors::RED_100,
                StatusColors::RED_700,
                StatusColors::RED_500,
                "Error",
            ),
            TaskStatus::Removed => (
                StatusColors::SLATE_100,
                StatusColors::SLATE_400,
                StatusColors::SLATE_400,
                "Removed",
            ),
        };

        let progress = task.progress();
        let pct = (progress * 100.).round() as u32;
        let subtitle = if let Some(err) = &task.error_message {
            err.clone()
        } else {
            let kind = if task.is_bt { "BT" } else { "HTTP" };
            if task.file_count > 1 {
                format!("{kind} · {} files", task.file_count)
            } else {
                format!("{kind} · 1 file")
            }
        };
        let speed_text = if task.download_speed > 0 {
            format_speed(task.download_speed)
        } else {
            "—".to_string()
        };
        let eta_text = match task.eta_seconds() {
            Some(secs) => format_eta(secs),
            None => "—".to_string(),
        };
        let size_text = if task.total_length > 0 {
            format_bytes(task.total_length)
        } else {
            "—".to_string()
        };

        let task_for_menu = task.clone();
        let entity = cx.entity();
        let task_for_open = task.clone();
        let gid_click = gid.clone();

        div()
            .id(SharedString::from(format!("task-{gid}")))
            .h(px(ROW_HEIGHT))
            .w_full()
            .overflow_hidden()
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(12.))
            .border_b_1()
            .border_color(border.opacity(0.5))
            .text_size(px(12.5))
            .when(selected, |this| this.bg(sel_bg))
            .when(!selected, |this| {
                this.hover(|s| s.bg(accent.opacity(0.15)))
            })
            .on_click(cx.listener({
                let task = task.clone();
                move |this, ev: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    if ev.click_count() >= 2 {
                        if task.status == TaskStatus::Complete {
                            this.open_task_folder(&task);
                        }
                        return;
                    }
                    let m = ev.modifiers();
                    let additive = m.platform || m.shift;
                    this.toggle_select(&gid_click, additive, cx);
                }
            }))
            .on_mouse_down(MouseButton::Right, cx.listener({
                let gid = gid.clone();
                move |this, _, _, cx| {
                    if !this.selected.contains(&gid) {
                        this.selected.clear();
                        this.selected.insert(gid.clone());
                        cx.notify();
                    }
                }
            }))
            .context_menu(move |menu, _w, _cx| {
                let t = task_for_menu.clone();
                let entity1 = entity.clone();
                let entity2 = entity.clone();
                let entity3 = entity.clone();
                let entity4 = entity.clone();
                let entity5 = entity.clone();
                let is_running = matches!(
                    t.status,
                    TaskStatus::Active | TaskStatus::Waiting | TaskStatus::Seeding
                );
                let is_paused = t.status == TaskStatus::Paused;
                let is_error = t.status == TaskStatus::Error;
                let gid1 = t.gid.clone();
                let gid2 = t.gid.clone();
                let status = t.status;
                let gid3 = t.gid.clone();
                let uri = t.uri.clone();
                let t_open = t.clone();
                let t_restart = t.clone();

                let mut menu = menu.min_w(px(200.));
                if is_running {
                    menu = menu.item(PopupMenuItem::new("Pause").on_click(move |_, _, cx| {
                        entity1.update(cx, |this, cx| this.pause_task(gid1.clone(), cx));
                    }));
                }
                if is_paused {
                    menu = menu.item(PopupMenuItem::new("Resume").on_click(move |_, _, cx| {
                        entity2.update(cx, |this, cx| this.resume_task(gid2.clone(), cx));
                    }));
                }
                if is_error {
                    menu = menu.item(PopupMenuItem::new("Restart").on_click(move |_, _, cx| {
                        entity5.update(cx, |this, cx| this.restart_task(&t_restart, cx));
                    }));
                }
                menu = menu
                    .item(PopupMenuItem::new("Open Folder").on_click(move |_, _, cx| {
                        entity3.update(cx, |this, _| this.open_task_folder(&t_open));
                    }))
                    .separator();
                if let Some(uri) = uri {
                    menu = menu.item(PopupMenuItem::new("Copy Link").on_click(move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(uri.clone()));
                    }));
                }
                let t_remove = t.clone();
                let _ = (gid3, status);
                menu.separator()
                    .item(PopupMenuItem::new("Remove…").on_click(move |_, window, cx| {
                        entity4.update(cx, |this, cx| {
                            this.confirm_remove(vec![t_remove.clone()], window, cx)
                        });
                    }))
            })
            // Name + subtitle
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(2.))
                    .overflow_hidden()
                    .child(
                        div()
                            .w_full()
                            .text_size(px(12.5))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(fg)
                            .truncate()
                            .child(task.name.clone()),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_size(px(11.))
                            .text_color(if task.error_message.is_some() {
                                gpui::rgb(StatusColors::RED_700).into()
                            } else {
                                muted
                            })
                            .truncate()
                            .child(subtitle),
                    ),
            )
            // Size
            .child(
                div()
                    .w(px(70.))
                    .flex_shrink_0()
                    .child(size_text),
            )
            // Progress
            .child(
                v_flex()
                    .w(px(120.))
                    .flex_shrink_0()
                    .gap(px(4.))
                    .child(
                        div()
                            .h(px(4.))
                            .w_full()
                            .rounded_full()
                            .bg(cx.theme().muted)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui::relative(progress.clamp(0., 1.)))
                                    .bg(gpui::rgb(bar_color)),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .text_size(px(10.5))
                            .text_color(muted)
                            .child(format!("{pct}%"))
                            .child(format_bytes(task.completed_length)),
                    ),
            )
            // Status pill
            .child(
                div().w(px(84.)).flex_shrink_0().child(
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded_full()
                        .bg(gpui::rgb(pill_bg))
                        .text_color(gpui::rgb(pill_fg))
                        .text_size(px(11.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .w_auto()
                        .flex()
                        .justify_center()
                        .child(pill_label),
                ),
            )
            // Down speed
            .child(
                div()
                    .w(px(84.))
                    .flex_shrink_0()
                    .child(speed_text),
            )
            // ETA
            .child(
                div()
                    .w(px(64.))
                    .flex_shrink_0()
                    .text_color(muted)
                    .child(eta_text),
            )
    }

    fn render_empty(&self, cx: &App) -> impl IntoElement {
        let query_active = !self.state.read(cx).search_query.trim().is_empty();
        let filter = self.state.read(cx).filter;
        let (title, hint) = if query_active {
            ("No matching tasks", "Try a different search")
        } else if filter != TaskFilter::All {
            ("No tasks match this filter", "Switch back to All Downloads")
        } else {
            ("No downloads yet", "Press + to add one")
        };

        div()
            .flex_1()
            .relative()
            .overflow_hidden()
            .child(mosaic_backdrop())
            .child(
                v_flex()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(cx.theme().muted_foreground)
                            .child(hint),
                    ),
            )
    }
}

/// Static approximation of Motrix's animated "cubic glass" empty-state gradient:
/// a mosaic of soft blue → pink squares fading out towards the edges.
fn mosaic_backdrop() -> impl IntoElement {
    const COLS: usize = 16;
    const ROWS: usize = 9;
    let blue = (87.0_f32, 128.0, 245.0);
    let pink = (245.0_f32, 161.0, 214.0);

    let mut rows = v_flex().absolute().bottom(px(-40.)).left_0().right_0().items_center();
    for r in 0..ROWS {
        let mut row = h_flex();
        for c in 0..COLS {
            let x = c as f32 / (COLS - 1) as f32;
            let y = r as f32 / (ROWS - 1) as f32;
            // Two soft blobs: blue on the left-center, pink right-center.
            let d_blue = ((x - 0.38).powi(2) * 2.4 + (y - 0.65).powi(2)).sqrt();
            let d_pink = ((x - 0.66).powi(2) * 2.4 + (y - 0.55).powi(2)).sqrt();
            let w_blue = (1.0 - d_blue * 2.2).max(0.);
            let w_pink = (1.0 - d_pink * 2.2).max(0.);
            let total = (w_blue + w_pink).min(1.0);
            let alpha = (total * 0.55).min(0.5);
            let (cr, cg, cb) = if w_blue + w_pink > 0. {
                let t = w_pink / (w_blue + w_pink);
                (
                    blue.0 + (pink.0 - blue.0) * t,
                    blue.1 + (pink.1 - blue.1) * t,
                    blue.2 + (pink.2 - blue.2) * t,
                )
            } else {
                blue
            };
            let color = gpui::Rgba {
                r: cr / 255.,
                g: cg / 255.,
                b: cb / 255.,
                a: alpha,
            };
            row = row.child(div().size(px(46.)).bg(color));
        }
        rows = rows.child(row);
    }
    rows
}

impl Render for DownloadsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tasks: Vec<Task> = {
            let st = self.state.read(cx);
            st.filtered_tasks().into_iter().cloned().collect()
        };
        let count = tasks.len();
        let entity = cx.entity();

        let list_body: gpui::AnyElement = if count == 0 {
            self.render_empty(cx).into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(
                    uniform_list("task-list", count, move |visible_range, window, cx| {
                        let mut rows = Vec::with_capacity(visible_range.len());
                        for ix in visible_range {
                            let task = &tasks[ix];
                            let row = entity.update(cx, |this, cx| {
                                this.render_row(task, cx).into_any_element()
                            });
                            rows.push(row);
                        }
                        let _ = window;
                        rows
                    })
                    .size_full()
                    .track_scroll(&self.scroll_handle),
                )
                .vertical_scrollbar(&self.scroll_handle)
                .into_any_element()
        };

        v_flex()
            .size_full()
            .child(self.render_header(window, cx))
            .children(self.render_selection_bar(cx))
            .child(
                v_flex()
                    .id("task-list-container")
                    .flex_1()
                    .min_h_0()
                    .mx(px(24.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_hidden()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.selected.is_empty() {
                            this.selected.clear();
                            cx.notify();
                        }
                    }))
                    .when(count > 0, |this| this.child(self.render_column_header(cx)))
                    .child(list_body),
            )
            .child(render_status_bar(&self.state, cx))
    }
}

/// Move a task's files to the user's Trash (permission-free `mv` into
/// ~/.Trash, deduplicated by timestamp suffix). Also removes aria2's
/// `.aria2` control file.
fn trash_task_files(task: &Task) {
    let mut paths: Vec<String> = Vec::new();
    if let Some(p) = &task.file_path {
        paths.push(p.clone());
        paths.push(format!("{p}.aria2"));
    }
    // Multi-file BT tasks live in dir/name/.
    if task.is_bt && !task.dir.is_empty() && !task.name.is_empty() && task.file_count > 1 {
        paths.push(format!("{}/{}", task.dir.trim_end_matches('/'), task.name));
    }
    std::thread::spawn(move || {
        let Some(home) = dirs::home_dir() else { return };
        let trash = home.join(".Trash");
        for p in paths {
            let src = std::path::PathBuf::from(&p);
            if !src.exists() {
                continue;
            }
            let name = src
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "download".into());
            let mut dest = trash.join(&name);
            if dest.exists() {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                dest = trash.join(format!("{stamp}-{name}"));
            }
            let _ = std::fs::rename(&src, &dest);
        }
    });
}
