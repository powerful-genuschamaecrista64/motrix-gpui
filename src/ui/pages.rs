use gpui::prelude::FluentBuilder;
use gpui::AppContext as _;
use gpui::{
    div, px, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, IconName, Sizable, WindowExt};

use crate::assets::{builtin_trackers, AppIcon};
use crate::state::AppState;
use crate::theme::StatusColors;

/// Shared page header: 24px semibold title with Motrix's panel padding.
pub fn page_header(title: &'static str) -> impl IntoElement {
    h_flex()
        .flex_shrink_0()
        .pt(px(32.))
        .px(px(24.))
        .pb(px(16.))
        .items_start()
        .justify_between()
        .child(
            div()
                .text_size(px(24.))
                .line_height(px(32.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
}

fn empty_hint(
    icon: Icon,
    title: &'static str,
    hint: &'static str,
    cx: &gpui::App,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .child(icon.size(px(28.)).text_color(cx.theme().muted_foreground))
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
        )
}

// ---- Trackers ----

const TRACKER_SOURCES: [(&str, &str); 2] = [
    (
        "best",
        "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best.txt",
    ),
    (
        "all",
        "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_all.txt",
    ),
];

pub struct TrackersPage {
    state: Entity<AppState>,
    trackers: Vec<String>,
    loading: bool,
    error: Option<String>,
}

impl TrackersPage {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let configured = state.read(cx).config.bt_trackers.clone();
        let trackers = if configured.is_empty() {
            // Same behavior as the Electron version: ship a built-in list.
            builtin_trackers()
        } else {
            configured
        };
        TrackersPage {
            state,
            trackers,
            loading: false,
            error: None,
        }
    }

    fn sync(&mut self, source_ix: usize, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = None;
        cx.notify();
        let url = TRACKER_SOURCES[source_ix].1.to_string();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let body: String = ureq::get(&url)
                        .timeout(std::time::Duration::from_secs(20))
                        .call()?
                        .into_string()?;
                    let list: Vec<String> = body
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    anyhow::Ok(list)
                })
                .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(list) if !list.is_empty() => {
                        this.trackers = list;
                        this.error = None;
                    }
                    Ok(_) => this.error = Some("Sync returned an empty list.".into()),
                    Err(err) => this.error = Some(format!("Sync failed: {err}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply(&self, window: &mut Window, cx: &mut Context<Self>) {
        let trackers = self.trackers.clone();
        let count = trackers.len();
        self.state.update(cx, |state, cx| {
            state.config.bt_trackers = trackers.clone();
            state.config.save();
            state.run_rpc(cx, move |client| {
                client.change_global_option(serde_json::json!({
                    "bt-tracker": trackers.join(","),
                }))
            });
            cx.notify();
        });
        window.push_notification(format!("{count} trackers applied to the engine."), cx);
    }
}

impl Render for TrackersPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let count = self.trackers.len();

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .pt(px(32.))
                    .px(px(24.))
                    .pb(px(16.))
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .line_height(px(32.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Trackers"),
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
                                    .child(count.to_string()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                Button::new("sync-best")
                                    .outline()
                                    .small()
                                    .loading(self.loading)
                                    .icon(Icon::new(AppIcon::RadioTower))
                                    .label("Sync best")
                                    .on_click(cx.listener(|this, _, _, cx| this.sync(0, cx))),
                            )
                            .child(
                                Button::new("sync-all")
                                    .outline()
                                    .small()
                                    .loading(self.loading)
                                    .label("Sync all")
                                    .on_click(cx.listener(|this, _, _, cx| this.sync(1, cx))),
                            )
                            .child(
                                Button::new("apply-trackers")
                                    .primary()
                                    .small()
                                    .label("Apply to engine")
                                    .disabled(count == 0)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply(window, cx)
                                    })),
                            ),
                    ),
            )
            .when_some(self.error.clone(), |this, err| {
                this.child(
                    div()
                        .mx(px(24.))
                        .mb(px(8.))
                        .px(px(12.))
                        .py(px(6.))
                        .rounded(px(8.))
                        .bg(gpui::rgb(StatusColors::RED_100))
                        .text_size(px(12.))
                        .text_color(gpui::rgb(StatusColors::RED_700))
                        .child(err),
                )
            })
            .map(|this| {
                if count == 0 {
                    this.child(empty_hint(
                        Icon::new(AppIcon::RadioTower),
                        "No tracker list yet",
                        "Sync the community tracker list, then apply it to the engine",
                        cx,
                    ))
                } else {
                    let trackers = self.trackers.clone();
                    this.child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .mx(px(24.))
                            .mb(px(16.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(border)
                            .overflow_hidden()
                            .child(
                                div()
                                    .id("trackers-scroll")
                                    .size_full()
                                    .overflow_y_scroll()
                                    .child(v_flex().py(px(4.)).children(
                                        trackers.into_iter().enumerate().map(|(ix, t)| {
                                            h_flex()
                                                .px(px(12.))
                                                .py(px(5.))
                                                .gap(px(10.))
                                                .when(ix % 2 == 1, |el| {
                                                    el.bg(cx.theme().muted.opacity(0.3))
                                                })
                                                .child(
                                                    div()
                                                        .w(px(28.))
                                                        .flex_shrink_0()
                                                        .text_size(px(10.))
                                                        .text_color(muted)
                                                        .child(format!("{}", ix + 1)),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .text_size(px(12.))
                                                        .font_family("Menlo")
                                                        .truncate()
                                                        .child(t),
                                                )
                                        }),
                                    )),
                            ),
                    )
                }
            })
    }
}

// ---- Plugins ----

pub struct PluginsPage;

impl PluginsPage {
    pub fn new() -> Self {
        PluginsPage
    }
}

impl Render for PluginsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(page_header("Plugins"))
            .child(empty_hint(
                Icon::new(AppIcon::ToyBrick),
                "No plugins installed",
                "Plugins are coming soon",
                cx,
            ))
    }
}

// ---- Notifications ----

pub struct NotificationsPage {
    state: Entity<AppState>,
}

impl NotificationsPage {
    pub fn new(state: Entity<AppState>) -> Self {
        NotificationsPage { state }
    }
}

impl Render for NotificationsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let events: Vec<_> = self
            .state
            .read(cx)
            .events
            .iter()
            .map(|e| (e.title, e.name.clone(), e.ok, e.time.clone()))
            .collect();

        v_flex()
            .size_full()
            .child(page_header("Notifications"))
            .map(|this| {
                if events.is_empty() {
                    this.child(empty_hint(
                        Icon::new(IconName::Bell),
                        "You're all caught up",
                        "Task notifications will show up here",
                        cx,
                    ))
                } else {
                    this.child(
                        div()
                            .id("notifications-scroll")
                            .flex_1()
                            .min_h_0()
                            .px(px(24.))
                            .pb(px(16.))
                            .overflow_y_scroll()
                            .child(v_flex().children(events.into_iter().map(
                                |(title, name, ok, time)| {
                                    h_flex()
                                        .px(px(4.))
                                        .py(px(10.))
                                        .gap(px(12.))
                                        .items_center()
                                        .border_b_1()
                                        .border_color(border.opacity(0.5))
                                        .child(
                                            div()
                                                .size(px(8.))
                                                .flex_shrink_0()
                                                .rounded_full()
                                                .bg(gpui::rgb(if ok {
                                                    StatusColors::GREEN_500
                                                } else {
                                                    StatusColors::RED_500
                                                })),
                                        )
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .gap(px(1.))
                                                .child(
                                                    div()
                                                        .text_size(px(13.))
                                                        .font_weight(gpui::FontWeight::MEDIUM)
                                                        .child(title),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(muted)
                                                        .truncate()
                                                        .child(name),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .text_size(px(11.))
                                                .text_color(muted)
                                                .child(time),
                                        )
                                },
                            ))),
                    )
                }
            })
    }
}
