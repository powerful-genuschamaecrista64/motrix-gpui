use std::path::PathBuf;

use base64::Engine;
use gpui::prelude::FluentBuilder;
use gpui::AppContext as _;
use gpui::{
    div, px, Context, Entity, InteractiveElement, IntoElement, ParentElement, PathPromptOptions,
    Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{
    h_flex, v_flex, ActiveTheme, Icon, IconName, Sizable, WindowExt,
};

use crate::assets::AppIcon;
use crate::state::AppState;

pub struct AddTaskForm {
    state: Entity<AppState>,
    active_tab: usize,
    urls: Entity<TextareaState>,
    filename: Entity<InputState>,
    split: Entity<InputState>,
    dir: String,
    torrent: Option<PathBuf>,
    advanced_open: bool,
}

pub struct AddTaskModal;

impl AddTaskModal {
    pub fn open<V: 'static>(state: Entity<AppState>, window: &mut Window, cx: &mut Context<V>) {
        let form = cx.new(|cx| AddTaskForm::new(state, window, cx));
        let form_for_ok = form.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let form = form.clone();
            let form_for_ok = form_for_ok.clone();
            dialog
                .rounded(cx.theme().radius_lg)
                .w(px(560.))
                .overlay(true)
                .overlay_closable(false)
                .title(
                    div()
                        .text_size(px(16.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("New Task"),
                )
                .child(form.clone())
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogClose::new().child(
                                h_flex().justify_end().child(Button::new("cancel").outline().small().label("Cancel")),
                            ),
                        )
                        .child(
                            DialogAction::new().child(
                                h_flex().justify_end().child(Button::new("download")
                                    .primary()
                                    .small()
                                    .label("Download")),
                            ),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    form_for_ok.update(cx, |form, cx| form.submit(window, cx))
                })
        });
    }
}

impl AddTaskForm {
    fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dir = state.read(cx).config.download_dir.clone();
        let urls = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder("Enter URL, magnet link…")
        });
        let filename = cx.new(|cx| InputState::new(window, cx).placeholder("Auto"));
        let split = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("16")
                .pattern(regex_lite())
        });
        AddTaskForm {
            state,
            active_tab: 0,
            urls,
            filename,
            split,
            dir,
            torrent: None,
            advanced_open: false,
        }
    }

    fn pick_dir(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = rx.await {
                if let Some(path) = paths.pop() {
                    this.update(cx, |form, cx| {
                        form.dir = path.to_string_lossy().to_string();
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn pick_torrent(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = rx.await {
                if let Some(path) = paths.pop() {
                    this.update(cx, |form, cx| {
                        form.torrent = Some(path);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Returns true when the dialog should close.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let dir = self.dir.clone();
        let out = self.filename.read(cx).value().trim().to_string();
        let split: u32 = self
            .split
            .read(cx)
            .value()
            .trim()
            .parse()
            .unwrap_or(0);

        let mut options = serde_json::json!({ "dir": dir });
        if !out.is_empty() {
            options["out"] = serde_json::json!(out);
        }
        if split > 0 {
            options["split"] = serde_json::json!(split.to_string());
        }

        if self.active_tab == 0 {
            let text = self.urls.read(cx).value().to_string();
            let urls: Vec<String> = text
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();
            if urls.is_empty() {
                window.push_notification("Enter at least one URL.", cx);
                return false;
            }
            let count = urls.len();
            self.state.update(cx, |state, cx| {
                state.run_rpc(cx, move |client| {
                    for url in urls {
                        client.add_uri(vec![url], options.clone())?;
                    }
                    Ok(())
                });
            });
            window.push_notification(
                format!(
                    "Added {count} task{}.",
                    if count == 1 { "" } else { "s" }
                ),
                cx,
            );
            true
        } else {
            let Some(path) = self.torrent.clone() else {
                window.push_notification("Choose a .torrent file first.", cx);
                return false;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                window.push_notification("Failed to read the torrent file.", cx);
                return false;
            };
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            self.state.update(cx, |state, cx| {
                state.run_rpc(cx, move |client| {
                    client.add_torrent(encoded, options)?;
                    Ok(())
                });
            });
            window.push_notification("Torrent task added.", cx);
            true
        }
    }

    fn render_dir_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        h_flex()
            .id("dir-picker")
            .w_full()
            .gap(px(10.))
            .items_center()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(px(8.))
            .px(px(12.))
            .py(px(8.))
            .hover(|this| this.border_color(cx.theme().ring))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(muted)
                    .flex_shrink_0()
                    .child("Save to"),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.))
                    .truncate()
                    .child(self.dir.clone()),
            )
            .child(
                Icon::new(IconName::Folder)
                    .size(px(16.))
                    .text_color(muted),
            )
            .on_click(cx.listener(|this, _, window, cx| this.pick_dir(window, cx)))
    }

    fn render_advanced(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap(px(8.))
            .child(
                h_flex()
                    .id("advanced-toggle")
                    .gap(px(6.))
                    .items_center()
                    .text_size(px(12.))
                    .text_color(muted)
                    .cursor_pointer()
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(px(14.))
                            .when(self.advanced_open, |icon| {
                                icon.rotate(gpui::Radians(std::f32::consts::FRAC_PI_2))
                            }),
                    )
                    .child("Advanced options")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.advanced_open = !this.advanced_open;
                        cx.notify();
                    })),
            )
            .when(self.advanced_open, |this| {
                this.child(
                    v_flex()
                        .ml(px(6.))
                        .pl(px(12.))
                        .border_l_2()
                        .border_color(cx.theme().border)
                        .gap(px(8.))
                        .child(advanced_row("Filename", Input::new(&self.filename).small(), cx))
                        .child(advanced_row("Connections", Input::new(&self.split).small(), cx)),
                )
            })
    }
}

fn advanced_row(
    label: &'static str,
    control: impl IntoElement,
    cx: &gpui::App,
) -> impl IntoElement {
    h_flex()
        .gap(px(12.))
        .items_center()
        .child(
            div()
                .w(px(80.))
                .flex_shrink_0()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(div().flex_1().child(control))
}

fn regex_lite() -> regex::Regex {
    regex::Regex::new(r"^\d*$").expect("valid regex")
}

impl Render for AddTaskForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;

        v_flex()
            .gap(px(12.))
            .pt(px(4.))
            .child(
                h_flex().child(
                    TabBar::new("add-task-tabs")
                        .w(px(180.))
                        .segmented()
                        .selected_index(self.active_tab)
                        .on_click(cx.listener(|this, ix: &usize, _, cx| {
                            this.active_tab = *ix;
                            cx.notify();
                        }))
                        .child(Tab::new().child(
                            h_flex()
                                .gap(px(6.))
                                .items_center()
                                .child(Icon::new(AppIcon::Link).size(px(14.)))
                                .child("Links"),
                        ))
                        .child(Tab::new().child(
                            h_flex()
                                .gap(px(6.))
                                .items_center()
                                .child(Icon::new(AppIcon::Magnet).size(px(14.)))
                                .child("Torrent"),
                        )),
                ),
            )
            .map(|this| {
                if self.active_tab == 0 {
                    this.child(Textarea::new(&self.urls).h(px(110.)))
                } else {
                    let label: gpui::SharedString = match &self.torrent {
                        Some(path) => path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "torrent file".into())
                            .into(),
                        None => "Click to select a .torrent file".into(),
                    };
                    this.child(
                        v_flex()
                            .id("torrent-drop")
                            .h(px(110.))
                            .w_full()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .rounded(px(8.))
                            .border_2()
                            .border_dashed()
                            .border_color(cx.theme().border)
                            .hover(|s| s.border_color(cx.theme().ring))
                            .cursor_pointer()
                            .child(
                                Icon::new(if self.torrent.is_some() {
                                    IconName::CircleCheck
                                } else {
                                    IconName::File
                                })
                                .size(px(24.))
                                .text_color(muted),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(muted)
                                    .child(label),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.pick_torrent(window, cx)),
                            ),
                    )
                }
            })
            .child(self.render_dir_row(cx))
            .child(self.render_advanced(cx))
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .text_size(px(11.))
                    .text_color(muted)
                    .child(Icon::new(AppIcon::Magnet).size(px(12.)))
                    .child("HTTP, HTTPS, FTP, magnet and .torrent are supported"),
            )
    }
}
