#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod activity;
mod aria2;
mod assets;
mod config;
mod notifications;
mod state;
mod theme;
mod ui;

use gpui::{
    px, size, App, AppContext, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use gpui_component::Root;

use crate::state::AppState;
use crate::ui::app::{AddTask, MotrixApp, Quit, ToggleSidebar};

fn main() {
    gpui_platform::application()
        .with_assets(assets::MotrixAssets)
        .run(move |cx| {
        gpui_component::init(cx);
        theme::init(cx);
        notifications::init();

        cx.bind_keys([
            KeyBinding::new("cmd-n", AddTask, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.set_menus(vec![
            Menu {
                name: "Motrix".into(),
                items: vec![MenuItem::action("Quit Motrix", Quit)],
                disabled: false,
            },
            Menu {
                name: "File".into(),
                items: vec![MenuItem::action("New Task…", AddTask)],
                disabled: false,
            },
        ]);

        let state = AppState::new(cx);

        cx.on_action({
            let state = state.clone();
            move |_: &Quit, cx| {
                let st = state.read(cx);
                let should_warn = st.config.warn_before_quit && st.has_running_tasks();
                if should_warn {
                    if let Some(window) = cx.active_window() {
                        let state = state.clone();
                        let _ = window.update(cx, move |_, window, cx| {
                            use gpui_component::WindowExt;
                            window.open_alert_dialog(cx, move |alert, _, _| {
                                let state = state.clone();
                                alert
                                    .title("Quit Motrix?")
                                    .description(
                                        "Downloads are still running. They will resume the next time you open Motrix.",
                                    )
                                    .show_cancel(true)
                                    .on_ok(move |_, _, cx| {
                                        state.update(cx, |state, _| state.shutdown());
                                        cx.quit();
                                        true
                                    })
                            });
                        });
                        return;
                    }
                }
                state.update(cx, |state, _| state.shutdown());
                cx.quit();
            }
        });

        let window_size = size(px(914.), px(700.));
        let bounds = Bounds::centered(None, window_size, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Motrix".into()),
                appears_transparent: true,
                traffic_light_position: Some(gpui::point(px(20.), px(20.))),
            }),
            window_background: WindowBackgroundAppearance::Blurred,
            window_min_size: Some(size(px(720.), px(520.))),
            kind: WindowKind::Normal,
            // The app owns title-bar dragging: only our explicit drag strips
            // move/zoom the window, so double-clicking chrome buttons never
            // triggers the macOS zoom gesture.
            app_owns_titlebar_drag: true,
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(options, |window, cx| {
                let app = cx.new(|cx| MotrixApp::new(state.clone(), window, cx));
                cx.new(|cx| Root::new(app, window, cx))
            })
            .expect("failed to open window");
            let _ = cx.update(|cx| cx.activate(true));
        })
        .detach();
    });
}
