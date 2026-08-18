use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

/// Serves our own bundled icons, falling back to gpui-component's assets.
pub struct MotrixAssets;

macro_rules! own_icons {
    ($($name:literal),* $(,)?) => {
        fn load_own(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $name, ".svg") =>
                    Some(include_bytes!(concat!("../assets/icons/", $name, ".svg"))),)*
                _ => None,
            }
        }
    };
}

own_icons!(
    "download",
    "upload",
    "radio-tower",
    "toy-brick",
    "trash-2",
    "rotate-ccw",
    "link-2",
    "magnet",
    "gauge",
    "list-checks",
    "square-check",
    "file-text",
);

/// Icon paths for our custom icons, usable with `Icon::new(...)` via `IconNamed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIcon {
    Download,
    Upload,
    RadioTower,
    ToyBrick,
    Trash,
    RotateCcw,
    Link,
    Magnet,
    Gauge,
    ListChecks,
    SquareCheck,
    FileText,
}

impl gpui_component::IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            AppIcon::Download => "icons/download.svg",
            AppIcon::Upload => "icons/upload.svg",
            AppIcon::RadioTower => "icons/radio-tower.svg",
            AppIcon::ToyBrick => "icons/toy-brick.svg",
            AppIcon::Trash => "icons/trash-2.svg",
            AppIcon::RotateCcw => "icons/rotate-ccw.svg",
            AppIcon::Link => "icons/link-2.svg",
            AppIcon::Magnet => "icons/magnet.svg",
            AppIcon::Gauge => "icons/gauge.svg",
            AppIcon::ListChecks => "icons/list-checks.svg",
            AppIcon::SquareCheck => "icons/square-check.svg",
            AppIcon::FileText => "icons/file-text.svg",
        }
        .into()
    }
}

impl AssetSource for MotrixAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = load_own(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}

/// Built-in BT tracker list (ngosang/trackerslist "best"), bundled at build
/// time like the Electron version's default trackers.
pub fn builtin_trackers() -> Vec<String> {
    include_str!("../assets/trackers_best.txt")
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}
