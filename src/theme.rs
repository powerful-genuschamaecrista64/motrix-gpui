use gpui::{hsla, rgb, rgba, App, Hsla, Window};
use gpui_component::theme::{Theme, ThemeMode};

use crate::config::ThemeMode as ConfigThemeMode;

/// Fixed status colors (same in light & dark, matching Motrix's status pills).
pub struct StatusColors;

impl StatusColors {
    pub const BLUE_100: u32 = 0xDBEAFE;
    pub const BLUE_800: u32 = 0x1E40AF;
    pub const BLUE_500: u32 = 0x3B82F6;
    pub const GREEN_100: u32 = 0xDCFCE7;
    pub const GREEN_700: u32 = 0x15803D;
    pub const GREEN_500: u32 = 0x22C55E;
    pub const AMBER_100: u32 = 0xFEF3C7;
    pub const AMBER_700: u32 = 0xB45309;
    pub const AMBER_500: u32 = 0xF59E0B;
    pub const RED_100: u32 = 0xFEE2E2;
    pub const RED_700: u32 = 0xB91C1C;
    pub const RED_500: u32 = 0xEF4444;
    pub const SLATE_100: u32 = 0xF1F5F9;
    pub const SLATE_600: u32 = 0x475569;
    pub const SLATE_400: u32 = 0x94A3B8;
    pub const GRAY_500: u32 = 0x6B7280;
    pub const YELLOW_500: u32 = 0xEAB308;
}

/// Download / upload accent colors from the Motrix dashboard.
pub const COLOR_DOWN: u32 = 0x16BCE9;
pub const COLOR_UP: u32 = 0x5D55FB;

pub fn init(cx: &mut App) {
    apply_palette(cx);
}

pub fn set_mode(mode: ConfigThemeMode, window: Option<&mut Window>, cx: &mut App) {
    match mode {
        ConfigThemeMode::Light => Theme::change(ThemeMode::Light, window, cx),
        ConfigThemeMode::Dark => Theme::change(ThemeMode::Dark, window, cx),
        ConfigThemeMode::System => Theme::sync_system_appearance(window, cx),
    }
    apply_palette(cx);
}

fn h(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn ha(hex: u32, alpha: f32) -> Hsla {
    let mut c: Hsla = rgb(hex).into();
    c.a = alpha;
    c
}

/// Override gpui-component's default theme with Motrix's palette.
pub fn apply_palette(cx: &mut App) {
    let dark = Theme::global(cx).is_dark();
    let theme = Theme::global_mut(cx);

    if !dark {
        theme.background = h(0xFFFFFF);
        theme.foreground = h(0x0A0A0A);
        theme.popover = h(0xFFFFFF);
        theme.popover_foreground = h(0x0A0A0A);
        theme.primary = h(0x171717);
        theme.primary_hover = h(0x2E2E2E);
        theme.primary_active = h(0x000000);
        theme.primary_foreground = h(0xFAFAFA);
        theme.secondary = h(0xF5F5F5);
        theme.secondary_hover = h(0xEBEBEB);
        theme.secondary_active = h(0xE0E0E0);
        theme.secondary_foreground = h(0x171717);
        theme.muted = h(0xF5F5F5);
        theme.muted_foreground = h(0x737373);
        theme.accent = h(0xF5F5F5);
        theme.accent_foreground = h(0x171717);
        theme.border = h(0xE5E5E5);
        theme.input = h(0xE5E5E5);
        theme.ring = h(0xA1A1A1);
        theme.danger = h(0xE7000B);
        theme.danger_foreground = h(0xFFFFFF);
        // Translucent sidebar over the blurred window background (macOS vibrancy).
        theme.sidebar = rgba(0xFFFFFF8C).into();
        theme.sidebar_foreground = h(0x0A0A0A);
        theme.sidebar_accent = hsla(0., 0., 0., 0.06);
        theme.sidebar_accent_foreground = h(0x171717);
        theme.sidebar_border = ha(0xE5E5E5, 0.5);
        theme.sidebar_primary = h(0x171717);
        theme.sidebar_primary_foreground = h(0xFAFAFA);
        theme.title_bar = hsla(0., 0., 0., 0.);
        theme.title_bar_border = hsla(0., 0., 0., 0.);
        theme.tab_bar = h(0xEEEEEE);
        theme.tab = hsla(0., 0., 0., 0.);
        theme.tab_active = h(0xFFFFFF);
        theme.tab_foreground = ha(0x0A0A0A, 0.6);
        theme.tab_active_foreground = h(0x0A0A0A);
        theme.progress_bar = h(StatusColors::BLUE_500);
        theme.scrollbar_thumb = hsla(0., 0., 0., 0.2);
        theme.chart_1 = h(COLOR_DOWN);
        theme.chart_2 = h(COLOR_UP);
    } else {
        theme.background = h(0x0A0A0A);
        theme.foreground = h(0xFAFAFA);
        theme.popover = h(0x171717);
        theme.popover_foreground = h(0xFAFAFA);
        theme.primary = h(0xE5E5E5);
        theme.primary_hover = h(0xD4D4D4);
        theme.primary_active = h(0xFFFFFF);
        theme.primary_foreground = h(0x171717);
        theme.secondary = h(0x262626);
        theme.secondary_hover = h(0x303030);
        theme.secondary_active = h(0x3A3A3A);
        theme.secondary_foreground = h(0xFAFAFA);
        theme.muted = h(0x262626);
        theme.muted_foreground = h(0xA1A1A1);
        theme.accent = h(0x262626);
        theme.accent_foreground = h(0xFAFAFA);
        theme.border = hsla(0., 0., 1., 0.10);
        theme.input = hsla(0., 0., 1., 0.15);
        theme.ring = h(0x737373);
        theme.danger = h(0xFF6467);
        theme.danger_foreground = h(0xFFFFFF);
        theme.sidebar = rgba(0x1717178C).into();
        theme.sidebar_foreground = h(0xFAFAFA);
        theme.sidebar_accent = hsla(0., 0., 1., 0.10);
        theme.sidebar_accent_foreground = h(0xFAFAFA);
        theme.sidebar_border = hsla(0., 0., 1., 0.08);
        theme.sidebar_primary = h(0x4436C9);
        theme.sidebar_primary_foreground = h(0xFAFAFA);
        theme.title_bar = hsla(0., 0., 0., 0.);
        theme.title_bar_border = hsla(0., 0., 0., 0.);
        theme.tab_bar = h(0x262626);
        theme.tab = hsla(0., 0., 0., 0.);
        theme.tab_active = h(0x0A0A0A);
        theme.tab_foreground = ha(0xFAFAFA, 0.6);
        theme.tab_active_foreground = h(0xFAFAFA);
        theme.progress_bar = h(StatusColors::BLUE_500);
        theme.scrollbar_thumb = hsla(0., 0., 1., 0.2);
        theme.chart_1 = h(0x2DC2EC);
        theme.chart_2 = h(0x7C76F9);
    }
    theme.radius = gpui::px(8.);
    theme.radius_lg = gpui::px(14.);
}

/// The main content "inset card" background (sidebar-inset token).
pub fn inset_bg(dark: bool) -> Hsla {
    if dark {
        ha(0x0A0A0A, 0.80)
    } else {
        ha(0xFFFFFF, 0.88)
    }
}
