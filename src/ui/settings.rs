use gpui::prelude::FluentBuilder;
use gpui::AppContext as _;
use gpui::{
    div, px, App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::switch::Switch;
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, IconName, Sizable, WindowExt};

use crate::assets::AppIcon;
use crate::config::ThemeMode;
use crate::state::AppState;
use crate::theme;
use crate::ui::pages::page_header;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Card {
    General,
    Appearance,
    Downloads,
    BitTorrent,
    Integration,
    Network,
    Advanced,
    About,
}

impl Card {
    fn title(&self) -> &'static str {
        match self {
            Card::General => "General",
            Card::Appearance => "Appearance",
            Card::Downloads => "Downloads",
            Card::BitTorrent => "BitTorrent",
            Card::Integration => "Integration",
            Card::Network => "Network",
            Card::Advanced => "Advanced",
            Card::About => "About",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Card::General => "Startup, default save folder, notifications",
            Card::Appearance => "Theme, language",
            Card::Downloads => "Concurrency, bandwidth limits, identity",
            Card::BitTorrent => "Trackers, seeding",
            Card::Integration => "Browser extensions, CLI, system integrations",
            Card::Network => "RPC endpoint and proxy",
            Card::Advanced => "Session persistence, maintenance",
            Card::About => "Version, source code and manual",
        }
    }

    fn icon(&self) -> (Icon, u32) {
        match self {
            Card::General => (Icon::new(IconName::Settings), 0x8B5CF6),
            Card::Appearance => (Icon::new(IconName::Palette), 0x3B82F6),
            Card::Downloads => (Icon::new(AppIcon::Download), 0xEF4444),
            Card::BitTorrent => (Icon::new(AppIcon::Magnet), 0x2563EB),
            Card::Integration => (Icon::new(AppIcon::ToyBrick), 0xE11D48),
            Card::Network => (Icon::new(IconName::Network), 0xF97316),
            Card::Advanced => (Icon::new(IconName::Settings2), 0x22C55E),
            Card::About => (Icon::new(IconName::Info), 0xEAB308),
        }
    }
}

pub struct SettingsPage {
    state: Entity<AppState>,
    active_card: Option<Card>,
    dir: Entity<InputState>,
    concurrent: Entity<InputState>,
    split: Entity<InputState>,
    dl_limit: Entity<InputState>,
    ul_limit: Entity<InputState>,
    ua: Entity<InputState>,
    port: Entity<InputState>,
    trackers: Entity<TextareaState>,
}

impl SettingsPage {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = state.read(cx).config.clone();
        let dir =
            cx.new(|cx| InputState::new(window, cx).default_value(config.download_dir.clone()));
        let concurrent = cx.new(|cx| {
            InputState::new(window, cx).default_value(config.max_concurrent_downloads.to_string())
        });
        let split =
            cx.new(|cx| InputState::new(window, cx).default_value(config.split.to_string()));
        let dl_limit = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0 = unlimited (KB/s)")
                .default_value(if config.max_overall_download_limit > 0 {
                    (config.max_overall_download_limit / 1024).to_string()
                } else {
                    String::new()
                })
        });
        let ul_limit = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0 = unlimited (KB/s)")
                .default_value(if config.max_overall_upload_limit > 0 {
                    (config.max_overall_upload_limit / 1024).to_string()
                } else {
                    String::new()
                })
        });
        let ua = cx.new(|cx| InputState::new(window, cx).default_value(config.user_agent.clone()));
        let port =
            cx.new(|cx| InputState::new(window, cx).default_value(config.rpc_port.to_string()));
        let trackers = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(6)
                .placeholder("One tracker URL per line")
                .default_value(config.bt_trackers.join("\n"))
        });

        SettingsPage {
            state,
            active_card: None,
            dir,
            concurrent,
            split,
            dl_limit,
            ul_limit,
            ua,
            port,
            trackers,
        }
    }

    fn apply_save_dir(&self, cx: &mut Context<Self>) {
        let dir = self.dir.read(cx).value().trim().to_string();
        if dir.is_empty() {
            return;
        }
        self.state.update(cx, |state, cx| {
            if state.config.download_dir == dir {
                return;
            }
            state.config.download_dir = dir.clone();
            state.config.save();
            state.run_rpc(cx, move |client| {
                client.change_global_option(serde_json::json!({ "dir": dir }))
            });
            cx.notify();
        });
    }

    fn apply_downloads(&self, window: &mut Window, cx: &mut Context<Self>) {
        let concurrent: u32 = self
            .concurrent
            .read(cx)
            .value()
            .trim()
            .parse()
            .unwrap_or(5)
            .clamp(1, 64);
        let split: u32 = self
            .split
            .read(cx)
            .value()
            .trim()
            .parse()
            .unwrap_or(16)
            .clamp(1, 64);
        let dl_kb: u64 = self.dl_limit.read(cx).value().trim().parse().unwrap_or(0);
        let ul_kb: u64 = self.ul_limit.read(cx).value().trim().parse().unwrap_or(0);
        let ua = self.ua.read(cx).value().trim().to_string();

        self.state.update(cx, |state, cx| {
            state.config.max_concurrent_downloads = concurrent;
            state.config.split = split;
            state.config.max_overall_download_limit = dl_kb * 1024;
            state.config.max_overall_upload_limit = ul_kb * 1024;
            state.config.user_agent = ua.clone();
            state.config.save();

            state.run_rpc(cx, move |client| {
                client.change_global_option(serde_json::json!({
                    "max-concurrent-downloads": concurrent.to_string(),
                    "max-overall-download-limit": (dl_kb * 1024).to_string(),
                    "max-overall-upload-limit": (ul_kb * 1024).to_string(),
                    "user-agent": ua,
                }))
            });
            cx.notify();
        });
        window.push_notification("Download settings saved.", cx);
    }

    fn apply_trackers(&self, window: &mut Window, cx: &mut Context<Self>) {
        let trackers: Vec<String> = self
            .trackers
            .read(cx)
            .value()
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
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
        window.push_notification("Tracker list saved.", cx);
    }

    // ---- Card grid ----

    fn render_card(&self, card: Card, cx: &mut Context<Self>) -> impl IntoElement {
        let (icon, tint) = card.icon();
        let mut tint_bg: gpui::Hsla = gpui::rgb(tint).into();
        tint_bg.a = 0.12;
        div()
            .id(SharedString::from(format!("card-{}", card.title())))
            .flex()
            .flex_col()
            .w_full()
            .h(px(180.))
            .px(px(20.))
            .py(px(16.))
            .rounded(px(12.))
            .overflow_hidden()
            .hover(|s| s.bg(cx.theme().muted.opacity(0.5)))
            .child(
                div()
                    .h(px(72.))
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_start()
                    .child(
                        div()
                            .size(px(56.))
                            .rounded(px(16.))
                            .bg(tint_bg)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon.size(px(28.)).text_color(gpui::rgb(tint))),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .pt(px(12.))
                    .gap(px(6.))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(card.title()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(gpui::rgb(0x9CA3AF))
                            .child(card.description()),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active_card = Some(card);
                cx.notify();
            }))
    }

    fn render_grid(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let border = cx.theme().border;
        let cards = [
            Card::General,
            Card::Appearance,
            Card::Downloads,
            Card::BitTorrent,
            Card::Integration,
            Card::Network,
            Card::Advanced,
            Card::About,
        ];

        let rows = cards.chunks(3).count();
        let mut grid = v_flex().px(px(24.)).pb(px(8.));
        for (row_ix, chunk) in cards.chunks(3).enumerate() {
            let mut row = h_flex().w_full().gap(px(8.)).py(px(8.));
            for card in chunk {
                row = row.child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .child(self.render_card(*card, cx)),
                );
            }
            for _ in chunk.len()..3 {
                row = row.child(div().flex_1().min_w(px(0.)));
            }
            if row_ix + 1 < rows {
                row = row.border_b_1().border_color(border.opacity(0.5));
            }
            grid = grid.child(row);
        }

        v_flex()
            .size_full()
            .child(page_header("Settings"))
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(grid),
            )
            .into_any_element()
    }

    // ---- Detail pages ----

    fn render_detail(&mut self, card: Card, cx: &mut Context<Self>) -> gpui::AnyElement {
        let body = match card {
            Card::General => self.render_general(cx),
            Card::Appearance => self.render_appearance(cx),
            Card::Downloads => self.render_downloads(cx),
            Card::BitTorrent => self.render_bittorrent(cx),
            Card::Network => self.render_network(cx),
            Card::Advanced => self.render_advanced(cx),
            Card::Integration => self.render_static(
                "Browser extension and CLI integration are coming soon.",
                cx,
            ),
            Card::About => self.render_about(cx),
        };

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .pt(px(28.))
                    .px(px(16.))
                    .pb(px(12.))
                    .items_center()
                    .gap(px(4.))
                    .child(
                        Button::new("settings-back")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronLeft)
                            .label("Settings")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_card = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(cx.theme().muted_foreground)
                            .child("/"),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .text_size(px(20.))
                            .line_height(px(28.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(card.title()),
                    ),
            )
            .child(
                div()
                    .id("settings-detail-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .px(px(24.))
                            .pb(px(24.))
                            .gap(px(2.))
                            .children([body]),
                    ),
            )
            .into_any_element()
    }

    fn render_general(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self.state.clone();
        let config = state.read(cx).config.clone();
        let entity = cx.entity();
        v_flex()
            .gap(px(2.))
            .child(settings_row(
                "Default save folder",
                "New tasks are saved here unless you pick another folder",
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(200.)).child(Input::new(&self.dir).small()))
                    .child(
                        Button::new("apply-dir")
                            .outline()
                            .small()
                            .label("Apply")
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.apply_save_dir(cx));
                            }),
                    )
                    .into_any_element(),
                cx,
            ))
            .child(switch_row(
                "resume-on-launch",
                "Resume downloads on launch",
                "Continue unfinished tasks when Motrix starts",
                config.resume_all_when_app_launched,
                {
                    let state = state.clone();
                    move |checked, _, cx| {
                        state.update(cx, |s, cx| {
                            s.config.resume_all_when_app_launched = checked;
                            s.config.save();
                            cx.notify();
                        });
                    }
                },
                cx,
            ))
            .child(switch_row(
                "notify-complete",
                "Notify on completion",
                "Show a system notification when a download finishes",
                config.notify_on_complete,
                {
                    let state = state.clone();
                    move |checked, _, cx| {
                        state.update(cx, |s, cx| {
                            s.config.notify_on_complete = checked;
                            s.config.save();
                            cx.notify();
                        });
                    }
                },
                cx,
            ))
            .child(switch_row(
                "notify-error",
                "Notify on failure",
                "Show a system notification when a download fails",
                config.notify_on_error,
                {
                    let state = state.clone();
                    move |checked, _, cx| {
                        state.update(cx, |s, cx| {
                            s.config.notify_on_error = checked;
                            s.config.save();
                            cx.notify();
                        });
                    }
                },
                cx,
            ))
            .child(switch_row(
                "warn-quit",
                "Warn before quitting",
                "Ask for confirmation while downloads are still running",
                config.warn_before_quit,
                {
                    let state = state.clone();
                    move |checked, _, cx| {
                        state.update(cx, |s, cx| {
                            s.config.warn_before_quit = checked;
                            s.config.save();
                            cx.notify();
                        });
                    }
                },
                cx,
            ))
            .into_any_element()
    }

    fn render_appearance(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self.state.clone();
        let current = state.read(cx).config.theme;
        v_flex()
            .gap(px(2.))
            .child(settings_row(
                "Theme",
                "Follow the system, or force light / dark",
                h_flex()
                    .gap(px(6.))
                    .children([
                        theme_button(
                            "theme-system",
                            "Auto",
                            ThemeMode::System,
                            current,
                            state.clone(),
                        ),
                        theme_button(
                            "theme-light",
                            "Light",
                            ThemeMode::Light,
                            current,
                            state.clone(),
                        ),
                        theme_button(
                            "theme-dark",
                            "Dark",
                            ThemeMode::Dark,
                            current,
                            state.clone(),
                        ),
                    ])
                    .into_any_element(),
                cx,
            ))
            .child(settings_row(
                "Language",
                "Interface language",
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child("English")
                    .into_any_element(),
                cx,
            ))
            .into_any_element()
    }

    fn render_downloads(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entity = cx.entity();
        v_flex()
            .gap(px(2.))
            .child(settings_row(
                "Max concurrent downloads",
                "How many tasks may download at the same time (1-64)",
                div()
                    .w(px(90.))
                    .child(Input::new(&self.concurrent).small())
                    .into_any_element(),
                cx,
            ))
            .child(settings_row(
                "Connections per task",
                "Segments each download is split into (1-64)",
                div()
                    .w(px(90.))
                    .child(Input::new(&self.split).small())
                    .into_any_element(),
                cx,
            ))
            .child(settings_row(
                "Download limit",
                "Global download speed cap, in KB/s (empty = unlimited)",
                div()
                    .w(px(140.))
                    .child(Input::new(&self.dl_limit).small())
                    .into_any_element(),
                cx,
            ))
            .child(settings_row(
                "Upload limit",
                "Global upload speed cap, in KB/s (empty = unlimited)",
                div()
                    .w(px(140.))
                    .child(Input::new(&self.ul_limit).small())
                    .into_any_element(),
                cx,
            ))
            .child(settings_row(
                "User-Agent",
                "Sent with HTTP downloads",
                div()
                    .w(px(220.))
                    .child(Input::new(&self.ua).small())
                    .into_any_element(),
                cx,
            ))
            .child(
                h_flex().justify_end().pt(px(12.)).child(
                    Button::new("save-downloads")
                        .primary()
                        .small()
                        .label("Save")
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| this.apply_downloads(window, cx));
                        }),
                ),
            )
            .into_any_element()
    }

    fn render_bittorrent(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entity = cx.entity();
        v_flex()
            .gap(px(10.))
            .pt(px(4.))
            .child(section_label("Tracker list", cx))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child("Announce URLs added to every BitTorrent task, one per line. Sync a fresh list from the Trackers page."),
            )
            .child(Textarea::new(&self.trackers).h(px(200.)))
            .child(
                h_flex().justify_end().child(
                    Button::new("save-trackers")
                        .primary()
                        .small()
                        .label("Save")
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| this.apply_trackers(window, cx));
                        }),
                ),
            )
            .into_any_element()
    }

    fn render_network(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let secret = self.state.read(cx).config.rpc_secret.clone();
        v_flex()
            .gap(px(2.))
            .child(settings_row(
                "RPC port",
                "Local aria2 JSON-RPC port. Takes effect after restarting Motrix",
                div()
                    .w(px(110.))
                    .child(Input::new(&self.port).small())
                    .into_any_element(),
                cx,
            ))
            .child(settings_row(
                "RPC secret",
                "Token required by external tools connecting to the engine",
                div()
                    .text_size(px(12.))
                    .font_family("Menlo")
                    .text_color(cx.theme().muted_foreground)
                    .child(if secret.is_empty() {
                        "(none)".to_string()
                    } else {
                        secret
                    })
                    .into_any_element(),
                cx,
            ))
            .into_any_element()
    }

    fn render_advanced(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self.state.clone();
        v_flex()
            .gap(px(2.))
            .child(settings_row(
                "Download history",
                "Remove all finished, failed and removed task records",
                Button::new("purge")
                    .outline()
                    .small()
                    .label("Clear history")
                    .on_click({
                        let state = state.clone();
                        move |_, window, cx| {
                            state.update(cx, |state, cx| {
                                state.run_rpc(cx, |client| client.purge_download_result());
                            });
                            window.push_notification("Download history cleared.", cx);
                        }
                    })
                    .into_any_element(),
                cx,
            ))
            .child(settings_row(
                "Configuration",
                "Config, session and activity files live here",
                Button::new("open-config")
                    .outline()
                    .small()
                    .label("Open folder")
                    .on_click(|_, _, _| {
                        if let Some(dir) = dirs::config_dir() {
                            let _ = open::that(dir.join("motrix-gpui"));
                        }
                    })
                    .into_any_element(),
                cx,
            ))
            .into_any_element()
    }

    fn render_about(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let version = self.state.read(cx).engine_version.clone();
        v_flex()
            .gap(px(6.))
            .pt(px(8.))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Motrix (GPUI rewrite)"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "Version 0.1.0 · aria2 {}",
                        if version.is_empty() {
                            "—".to_string()
                        } else {
                            format!("v{version}")
                        }
                    )),
            )
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child("A full-featured download manager. Built with Rust + GPUI."),
            )
            .child(
                v_flex()
                    .pt(px(16.))
                    .gap(px(6.))
                    .child(section_label("Credits", cx))
                    .children(
                        [
                            ("Motrix", "https://github.com/agalwood/Motrix"),
                            ("aria2", "https://github.com/aria2/aria2"),
                            ("GPUI (Zed)", "https://github.com/zed-industries/zed"),
                            (
                                "gpui-component",
                                "https://github.com/longbridge/gpui-component",
                            ),
                            ("Lucide Icons", "https://github.com/lucide-icons/lucide"),
                            (
                                "trackerslist",
                                "https://github.com/ngosang/trackerslist",
                            ),
                        ]
                        .map(|(name, url)| {
                            h_flex()
                                .id(SharedString::from(format!("credit-{name}")))
                                .gap(px(8.))
                                .items_center()
                                .text_size(px(12.))
                                .cursor_pointer()
                                .child(
                                    div()
                                        .w(px(110.))
                                        .flex_shrink_0()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate()
                                        .child(url),
                                )
                                .on_click(move |_, _, cx| cx.open_url(url))
                        }),
                    ),
            )
            .into_any_element()
    }

    fn render_static(&self, text: &'static str, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .pt(px(8.))
            .text_size(px(13.))
            .text_color(cx.theme().muted_foreground)
            .child(text)
            .into_any_element()
    }
}

fn section_label(text: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(12.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(cx.theme().muted_foreground)
        .child(text.to_uppercase())
}

/// One settings row in the original Motrix style: title + description on the
/// left, the control on the right.
fn settings_row(
    title: &'static str,
    description: &'static str,
    control: gpui::AnyElement,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .gap(px(16.))
        .py(px(10.))
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.5))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(description),
                ),
        )
        .child(div().flex_shrink_0().child(control))
}

fn switch_row(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    checked: bool,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    settings_row(
        title,
        description,
        Switch::new(id)
            .checked(checked)
            .on_click(move |checked: &bool, window, cx| on_toggle(*checked, window, cx))
            .into_any_element(),
        cx,
    )
}

fn theme_button(
    id: &'static str,
    label: &'static str,
    mode: ThemeMode,
    current: ThemeMode,
    state: Entity<AppState>,
) -> Button {
    Button::new(id)
        .small()
        .map(|b| if mode == current { b.primary() } else { b.outline() })
        .label(label)
        .on_click(move |_, window, cx| {
            theme::set_mode(mode, Some(window), cx);
            state.update(cx, |state, cx| {
                state.config.theme = mode;
                state.config.save();
                cx.notify();
            });
        })
}

impl Render for SettingsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_card {
            Some(card) => self.render_detail(card, cx),
            None => self.render_grid(cx),
        }
    }
}
