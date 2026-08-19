use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::chart::AreaChart;
use gpui_component::{h_flex, v_flex, ActiveTheme};

use crate::activity::{civil_from_days, today};
use crate::aria2::format_bytes;
use crate::aria2::TaskStatus;
use crate::state::{AppState, EngineStatus};
use crate::theme::{StatusColors, COLOR_DOWN, COLOR_UP};

pub struct Dashboard {
    state: Entity<AppState>,
}

impl Dashboard {
    pub fn new(state: Entity<AppState>) -> Self {
        Dashboard { state }
    }
}

fn tile(cx: &App) -> gpui::Div {
    let dark = cx.theme().is_dark();
    div()
        .rounded(px(18.))
        .border_1()
        .border_color(if dark {
            gpui::hsla(0., 0., 1., 0.10)
        } else {
            gpui::hsla(0., 0., 0., 0.07)
        })
        .bg(if dark {
            gpui::rgba(0x212121F2)
        } else {
            gpui::rgba(0xFFFFFFF5)
        })
        .p(px(14.))
        .overflow_hidden()
}

fn tile_label(text: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(cx.theme().muted_foreground)
        .child(text.to_uppercase())
}

fn speed_number(value: String, cx: &App) -> impl IntoElement {
    h_flex()
        .items_baseline()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(26.))
                .line_height(px(30.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(value),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .child("/s"),
        )
}

/// A speed-limit preset chip.
fn limit_chip(
    id: &'static str,
    label: &'static str,
    limit: u64,
    current: u64,
    state: Entity<AppState>,
) -> impl IntoElement {
    let active = current == limit;
    div()
        .id(SharedString::new_static(id))
        .px(px(7.))
        .py(px(3.))
        .rounded(px(7.))
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .map(|this| {
            if active {
                this.bg(gpui::rgb(0x3B82F6)).text_color(gpui::white())
            } else {
                this.bg(gpui::hsla(0., 0., 0.5, 0.10))
            }
        })
        .cursor_pointer()
        .child(label)
        .on_click(move |_, _, cx| {
            state.update(cx, |state, cx| {
                state.config.max_overall_download_limit = limit;
                state.config.save();
                state.run_rpc(cx, move |client| {
                    client.change_global_option(serde_json::json!({
                        "max-overall-download-limit": limit.to_string(),
                    }))
                });
                cx.notify();
            });
        })
}

const HEAT_COLORS_LIGHT: [u32; 5] = [0xEBEDF0, 0xB7E9F7, 0x6ED4EE, 0x2CB8DF, 0x0E92B8];
const HEAT_COLORS_DARK: [u32; 5] = [0x2A2A2A, 0x114C5C, 0x17708A, 0x1D96B8, 0x2DC2EC];

fn heat_level(bytes: u64) -> usize {
    match bytes {
        0 => 0,
        b if b < 10 * 1024 * 1024 => 1,
        b if b < 100 * 1024 * 1024 => 2,
        b if b < 1024 * 1024 * 1024 => 3,
        _ => 4,
    }
}

/// Speed sparkline: an area chart when there is data, a flat baseline when idle.
fn speed_chart(
    data: Vec<(usize, f64)>,
    has_data: bool,
    color: u32,
) -> gpui::AnyElement {
    use gpui::IntoElement as _;
    if has_data {
        div()
            .size_full()
            .child(
                AreaChart::new(data)
                    .x(|d: &(usize, f64)| d.0.to_string())
                    .y(|d: &(usize, f64)| d.1)
                    .stroke(gpui::rgb(color))
                    .fill({
                        let mut c: gpui::Hsla = gpui::rgb(color).into();
                        c.a = 0.15;
                        c
                    })
                    .natural()
                    .x_axis(false)
                    .grid(false),
            )
            .into_any_element()
    } else {
        div()
            .h(px(2.))
            .w_full()
            .rounded_full()
            .bg({
                let mut c: gpui::Hsla = gpui::rgb(color).into();
                c.a = 0.35;
                c
            })
            .into_any_element()
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.state.read(cx);
        let stat = st.stat;
        let engine = st.engine_status;
        let engine_missing = st.aria2_missing();
        let version = st.engine_version.clone();
        let dl_limit = st.config.max_overall_download_limit;
        let today_bytes = st.activity.today_bytes();
        let total_bytes = st.activity.total_bytes();

        let mut downloading = 0;
        let mut waiting = 0;
        let mut seeding = 0;
        for t in &st.tasks {
            match t.status {
                TaskStatus::Active => downloading += 1,
                TaskStatus::Waiting | TaskStatus::Paused => waiting += 1,
                TaskStatus::Seeding => seeding += 1,
                _ => {}
            }
        }
        let active_total = downloading + seeding;

        let (engine_label, dot) = match engine {
            EngineStatus::Ready => ("Ready", StatusColors::GREEN_500),
            EngineStatus::Starting => ("Starting", StatusColors::BLUE_500),
            EngineStatus::Offline => ("Offline", StatusColors::RED_500),
        };
        let limit_text: SharedString = match dl_limit {
            0 => "Standard mode".into(),
            n => format!("Limited to {}/s", format_bytes(n)).into(),
        };

        let muted = cx.theme().muted_foreground;
        let dark = cx.theme().is_dark();
        let state = self.state.clone();
        let up_has_data = st.up_history.iter().any(|v| *v > 0);
        let down_has_data = st.down_history.iter().any(|v| *v > 0);
        let down_data: Vec<(usize, f64)> = st
            .down_history
            .iter()
            .enumerate()
            .map(|(i, v)| (i, *v as f64))
            .collect();
        let up_data: Vec<(usize, f64)> = st
            .up_history
            .iter()
            .enumerate()
            .map(|(i, v)| (i, *v as f64))
            .collect();

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .pt(px(32.))
                    .px(px(24.))
                    .pb(px(16.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .line_height(px(32.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Dashboard"),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .px(px(24.))
                    .pb(px(16.))
                    .gap(px(12.))
                    // Row 1: Engine / Speed limit / Upload / Download
                    .child(
                        h_flex()
                            .h(px(150.))
                            .flex_shrink_0()
                            .gap(px(12.))
                            .items_stretch()
                            .child(
                                tile(cx)
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .child(tile_label("Engine", cx))
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(engine_label),
                                    )
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(muted)
                                                    .flex_1()
                                                    .min_w(px(0.))
                                                    .truncate()
                                                    .child(if engine_missing {
                                                        format!(
                                                            "aria2c not found · {}",
                                                            crate::aria2::install_hint()
                                                        )
                                                    } else if version.is_empty() {
                                                        "aria2".to_string()
                                                    } else {
                                                        format!("aria2 v{version}")
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .size(px(10.))
                                                    .rounded_full()
                                                    .bg(gpui::rgb(dot)),
                                            ),
                                    ),
                            )
                            .child(
                                tile(cx)
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .child(tile_label("Speed limit", cx))
                                    .child(
                                        h_flex()
                                            .gap(px(6.))
                                            .child(limit_chip(
                                                "limit-off",
                                                "Full",
                                                0,
                                                dl_limit,
                                                state.clone(),
                                            ))
                                            .child(limit_chip(
                                                "limit-10m",
                                                "10M",
                                                10 * 1024 * 1024,
                                                dl_limit,
                                                state.clone(),
                                            ))
                                            .child(limit_chip(
                                                "limit-1m",
                                                "1M",
                                                1024 * 1024,
                                                dl_limit,
                                                state.clone(),
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(muted)
                                            .child(limit_text),
                                    ),
                            )
                            .child(
                                tile(cx)
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .child(tile_label("Upload", cx))
                                    .child(div().h(px(4.)))
                                    .child(speed_number(format_bytes(stat.upload_speed), cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_h(px(36.))
                                            .mt(px(6.))
                                            .flex()
                                            .items_end()
                                            .child(speed_chart(up_data, up_has_data, COLOR_UP)),
                                    ),
                            )
                            .child(
                                tile(cx)
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .child(tile_label("Download", cx))
                                    .child(div().h(px(4.)))
                                    .child(speed_number(format_bytes(stat.download_speed), cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_h(px(36.))
                                            .mt(px(6.))
                                            .flex()
                                            .items_end()
                                            .child(speed_chart(
                                                down_data,
                                                down_has_data,
                                                COLOR_DOWN,
                                            )),
                                    ),
                            ),
                    )
                    // Row 2: Active tasks / Transfer
                    .child(
                        h_flex()
                            .h(px(140.))
                            .flex_shrink_0()
                            .gap(px(12.))
                            .items_stretch()
                            .child(
                                tile(cx)
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .child(tile_label("Active tasks", cx))
                                            .child(
                                                div()
                                                    .text_size(px(22.))
                                                    .line_height(px(24.))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child(active_total.to_string()),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap(px(24.))
                                            .child(stat_mini(downloading, "Downloading", cx))
                                            .child(stat_mini(waiting, "Waiting", cx))
                                            .child(stat_mini(seeding, "Seeding", cx)),
                                    ),
                            )
                            .child(
                                tile(cx)
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .child(tile_label("Transfer today", cx))
                                    .child(
                                        div()
                                            .text_size(px(22.))
                                            .line_height(px(24.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(format_bytes(today_bytes)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(muted)
                                            .child(format!(
                                                "All time · {}",
                                                format_bytes(total_bytes)
                                            )),
                                    ),
                            ),
                    )
                    // Row 3: Activity heatmap
                    .child(
                        tile(cx)
                            .h(px(220.))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(tile_label("Activity", cx))
                            .child(
                                div()
                                    .flex_1()
                                    .pt(px(8.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(self.render_heatmap(dark, cx)),
                            )
                            .child(
                                h_flex()
                                    .justify_end()
                                    .items_center()
                                    .gap(px(4.))
                                    .text_size(px(10.))
                                    .text_color(muted)
                                    .child("Less")
                                    .children((0..5).map(|i| {
                                        let colors = if dark {
                                            HEAT_COLORS_DARK
                                        } else {
                                            HEAT_COLORS_LIGHT
                                        };
                                        div()
                                            .size(px(9.))
                                            .rounded(px(2.))
                                            .bg(gpui::rgb(colors[i]))
                                    }))
                                    .child("More"),
                            ),
                    ),
            )
    }
}

impl Dashboard {
    fn render_heatmap(&self, dark: bool, cx: &mut Context<Self>) -> impl IntoElement {
        const WEEKS: usize = 40;
        let colors = if dark {
            HEAT_COLORS_DARK
        } else {
            HEAT_COLORS_LIGHT
        };
        let st = self.state.read(cx);
        let today = today();
        // Sunday-of-current-week: dow 0 = Sunday for (day + 4) % 7.
        let dow_today = ((today + 4) % 7) as i64;
        let current_sunday = today - dow_today;
        let start_sunday = current_sunday - (WEEKS as i64 - 1) * 7;
        let muted = cx.theme().muted_foreground;

        // Month labels: mark the column where a new month starts.
        let mut month_row = h_flex().gap(px(3.)).pl(px(26.));
        let mut last_month = 0;
        let mut columns = h_flex().gap(px(3.));

        for week in 0..WEEKS {
            let week_start = start_sunday + week as i64 * 7;
            let (_, month, _) = civil_from_days(week_start);
            let label: SharedString = if month != last_month {
                last_month = month;
                MONTHS[(month - 1) as usize].into()
            } else {
                "".into()
            };
            // 11px column + 3px gap; labels overflow into the empty
            // neighbours like GitHub's contribution graph.
            month_row = month_row.child(
                div()
                    .w(px(11.))
                    .flex_shrink_0()
                    .text_size(px(9.))
                    .text_color(muted)
                    .whitespace_nowrap()
                    .child(label),
            );

            let mut col = v_flex().gap(px(3.));
            for dow in 0..7 {
                let day = week_start + dow;
                let cell = if day > today {
                    div().size(px(11.)).rounded(px(2.5))
                } else {
                    let bytes = st.activity.bytes_on(day);
                    div()
                        .size(px(11.))
                        .rounded(px(2.5))
                        .bg(gpui::rgb(colors[heat_level(bytes)]))
                };
                col = col.child(cell);
            }
            columns = columns.child(col);
        }

        let day_labels = v_flex()
            .gap(px(3.))
            .pt(px(0.))
            .children([("", 0), ("Mon", 1), ("", 2), ("Wed", 3), ("", 4), ("Fri", 5), ("", 6)].map(
                |(label, _)| {
                    div()
                        .h(px(11.))
                        .w(px(23.))
                        .text_size(px(9.))
                        .text_color(muted)
                        .overflow_hidden()
                        .child(label)
                },
            ));

        v_flex()
            .id("activity-heatmap")
            .overflow_x_scroll()
            .gap(px(4.))
            .child(month_row)
            .child(h_flex().gap(px(3.)).items_start().child(day_labels).child(columns))
    }
}

fn stat_mini(value: usize, label: &'static str, cx: &App) -> impl IntoElement {
    v_flex()
        .gap(px(1.))
        .child(
            div()
                .text_size(px(18.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(cx.theme().muted_foreground)
                .child(label.to_uppercase()),
        )
}
