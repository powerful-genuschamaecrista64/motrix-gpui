use gpui::{div, px, App, Entity, IntoElement, ParentElement, Styled};
use gpui_component::{h_flex, ActiveTheme, Icon, IconName};

use crate::aria2::{format_speed, GlobalStat};
use crate::state::{AppState, EngineStatus};
use crate::theme::StatusColors;

fn speed_cell(icon: IconName, value: String, cx: &App) -> impl IntoElement {
    h_flex()
        .gap(px(4.))
        .items_center()
        .text_size(px(11.5))
        .text_color(cx.theme().muted_foreground)
        .child(Icon::new(icon).size(px(12.)))
        .child(
            div()
                .text_color(cx.theme().foreground)
                .child(value),
        )
}

fn engine_badge(status: EngineStatus, cx: &App) -> impl IntoElement {
    let (dot, label) = match status {
        EngineStatus::Starting => (StatusColors::BLUE_500, "Engine starting"),
        EngineStatus::Ready => (StatusColors::GREEN_500, "Engine ready"),
        EngineStatus::Offline => (StatusColors::RED_500, "Engine offline"),
    };
    h_flex()
        .h(px(20.))
        .px(px(8.))
        .gap(px(8.))
        .items_center()
        .rounded(px(26.))
        .bg(cx.theme().secondary)
        .text_size(px(12.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(cx.theme().secondary_foreground)
        .child(
            div()
                .size(px(8.))
                .rounded_full()
                .bg(gpui::rgb(dot)),
        )
        .child(label)
}

/// Bottom status bar: speeds + counts on the left, engine badge on the right.
pub fn render_status_bar(state: &Entity<AppState>, cx: &App) -> impl IntoElement {
    let st = state.read(cx);
    let stat: GlobalStat = st.stat;
    let (active, completed, error) = st.counts();
    let engine = st.engine_status;

    h_flex()
        .flex_shrink_0()
        .px(px(24.))
        .py(px(12.))
        .items_center()
        .justify_between()
        .gap(px(8.))
        .child(
            h_flex()
                .gap(px(16.))
                .items_center()
                .child(speed_cell(
                    IconName::ArrowDown,
                    format_speed(stat.download_speed),
                    cx,
                ))
                .child(speed_cell(
                    IconName::ArrowUp,
                    format_speed(stat.upload_speed),
                    cx,
                ))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "Active {active} · Completed {completed} · Error {error}"
                        )),
                ),
        )
        .child(engine_badge(engine, cx))
}
