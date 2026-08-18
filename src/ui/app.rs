use gpui::prelude::FluentBuilder;
use gpui::AppContext as _;
use gpui::{
    actions, div, px, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, WindowControlArea,
};
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, IconName, Root};

use crate::assets::AppIcon;
use crate::state::{AppState, Route};
use crate::theme;
use crate::ui::add_task::AddTaskModal;
use crate::ui::dashboard::Dashboard;
use crate::ui::downloads::DownloadsPage;
use crate::ui::pages::{NotificationsPage, PluginsPage, TrackersPage};
use crate::ui::settings::SettingsPage;

actions!(motrix, [AddTask, Quit, ToggleSidebar]);

pub const CHROME_HEIGHT: f32 = 40.;

pub struct MotrixApp {
    state: Entity<AppState>,
    downloads: Entity<DownloadsPage>,
    dashboard: Entity<Dashboard>,
    settings: Entity<SettingsPage>,
    trackers: Entity<TrackersPage>,
    plugins: Entity<PluginsPage>,
    notifications: Entity<NotificationsPage>,
    sidebar_open: bool,
}

impl MotrixApp {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        // Apply the configured theme now that we have a window.
        let mode = state.read(cx).config.theme;
        theme::set_mode(mode, Some(window), cx);

        let downloads = cx.new(|cx| DownloadsPage::new(state.clone(), window, cx));
        let dashboard = cx.new(|_| Dashboard::new(state.clone()));
        let settings = cx.new(|cx| SettingsPage::new(state.clone(), window, cx));
        let trackers = cx.new(|cx| TrackersPage::new(state.clone(), window, cx));
        let plugins = cx.new(|_| PluginsPage::new());
        let notifications = cx.new(|_| NotificationsPage::new(state.clone()));

        MotrixApp {
            state,
            downloads,
            dashboard,
            settings,
            trackers,
            plugins,
            notifications,
            sidebar_open: true,
        }
    }

    fn open_add_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        AddTaskModal::open(self.state.clone(), window, cx);
    }

    fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.route = route;
            cx.notify();
        });
    }

    fn nav_item(
        &self,
        id: &'static str,
        icon: Icon,
        label: &'static str,
        route: Route,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.state.read(cx).route == route;
        let accent = cx.theme().sidebar_accent;
        let fg = cx.theme().sidebar_foreground;
        div()
            .id(SharedString::new_static(id))
            .h(px(38.))
            .w_full()
            .px(px(10.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(14.))
            .text_color(fg)
            .when(active, |this| this.bg(accent))
            .hover(|this| this.bg(accent))
            .child(icon.size(px(16.)).flex_shrink_0())
            .child(div().flex_1().truncate().child(label))
            .on_click(cx.listener(move |this, _, _, cx| this.set_route(route, cx)))
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().sidebar_border;
        v_flex()
            .w(px(200.))
            .h_full()
            .flex_shrink_0()
            .p(px(8.))
            // Clear the window chrome / traffic lights.
            .pt(px(53.))
            .gap(px(4.))
            .child(self.nav_item(
                "nav-dashboard",
                Icon::new(IconName::LayoutDashboard),
                "Dashboard",
                Route::Dashboard,
                cx,
            ))
            .child(self.nav_item(
                "nav-downloads",
                Icon::new(AppIcon::Download),
                "Downloads",
                Route::Downloads,
                cx,
            ))
            .child(self.nav_item(
                "nav-trackers",
                Icon::new(AppIcon::RadioTower),
                "Trackers",
                Route::Trackers,
                cx,
            ))
            .child(self.nav_item(
                "nav-plugins",
                Icon::new(AppIcon::ToyBrick),
                "Plugins",
                Route::Plugins,
                cx,
            ))
            .child(div().flex_1())
            .child(self.nav_item(
                "nav-notifications",
                Icon::new(IconName::Bell),
                "Notifications",
                Route::Notifications,
                cx,
            ))
            .child(
                div()
                    .h(px(1.))
                    .mx(px(8.))
                    .my(px(4.))
                    .bg(border),
            )
            .child(self.nav_item(
                "nav-settings",
                Icon::new(IconName::Settings),
                "Settings",
                Route::Settings,
                cx,
            ))
    }

    fn chrome_button(
        &self,
        id: &'static str,
        icon: impl Into<Icon>,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let accent = cx.theme().sidebar_accent;
        let fg = cx.theme().sidebar_foreground;
        div()
            .id(SharedString::new_static(id))
            .size(px(28.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .justify_center()
            .text_color(fg.opacity(0.5))
            .hover(|this| this.bg(accent).text_color(fg.opacity(0.75)))
            .child(icon.into().size(px(16.)))
            .on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)))
    }

    fn render_chrome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Only the empty strips are draggable, so double-clicking the buttons
        // never triggers the macOS title-bar zoom gesture.
        h_flex()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(CHROME_HEIGHT))
            .items_end()
            .child(
                div()
                    .w(px(93.))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(px(6.))
                    .pb(px(0.))
                    .child(self.chrome_button(
                        "chrome-sidebar",
                        IconName::PanelLeft,
                        cx,
                        |this, _, cx| {
                            this.sidebar_open = !this.sidebar_open;
                            cx.notify();
                        },
                    ))
                    .child(self.chrome_button(
                        "chrome-add",
                        IconName::Plus,
                        cx,
                        |this, window, cx| {
                            this.open_add_task(window, cx);
                        },
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
    }
}

impl Render for MotrixApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let dark = cx.theme().is_dark();
        let route = self.state.read(cx).route;

        let page = match route {
            Route::Dashboard => self.dashboard.clone().into_any_element(),
            Route::Downloads => self.downloads.clone().into_any_element(),
            Route::Trackers => self.trackers.clone().into_any_element(),
            Route::Plugins => self.plugins.clone().into_any_element(),
            Route::Notifications => self.notifications.clone().into_any_element(),
            Route::Settings => self.settings.clone().into_any_element(),
        };

        let transparent_inset = route == Route::Dashboard;

        h_flex()
            .size_full()
            .relative()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().foreground)
            .font_family(".SystemUIFont")
            .on_action(cx.listener(|this, _: &AddTask, window, cx| {
                this.open_add_task(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar_open = !this.sidebar_open;
                cx.notify();
            }))
            .when(self.sidebar_open, |this| {
                this.child(self.render_sidebar(cx))
            })
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .p(px(8.))
                    .when(self.sidebar_open, |this| this.pl(px(0.)))
                    .child(
                        div()
                            .size_full()
                            .rounded(px(14.))
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .map(|this| {
                                if transparent_inset {
                                    this
                                } else {
                                    this.bg(theme::inset_bg(dark)).shadow_sm()
                                }
                            })
                            // With the sidebar hidden the window chrome floats over
                            // the content card, so push the page below it.
                            .when(!self.sidebar_open, |this| this.pt(px(28.)))
                            .child(page),
                    ),
            )
            .child(self.render_chrome(cx))
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}
