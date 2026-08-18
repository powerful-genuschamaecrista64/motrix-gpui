//! System notifications.
//!
//! On macOS this uses UNUserNotificationCenter (requires running from a .app
//! bundle — see scripts/bundle-macos.sh). The first call to `init` triggers
//! the system permission prompt. Outside a bundle it falls back to
//! AppleScript. Other platforms go through notify-rust (Windows toasts,
//! Linux DBus).

#[cfg(target_os = "macos")]
mod macos {
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };

    pub fn is_bundled() -> bool {
        unsafe { NSBundle::mainBundle().bundleIdentifier().is_some() }
    }

    /// Ask for notification permission (shows the macOS prompt on first run).
    pub fn request_permission() {
        if !is_bundled() {
            return;
        }
        unsafe {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            let options = UNAuthorizationOptions::Alert
                | UNAuthorizationOptions::Sound
                | UNAuthorizationOptions::Badge;
            let handler = block2::StackBlock::new(
                |_granted: objc2::runtime::Bool, _error: *mut NSError| {},
            );
            center.requestAuthorizationWithOptions_completionHandler(
                options,
                &handler.copy(),
            );
        }
    }

    pub fn notify(title: &str, body: &str) -> bool {
        if !is_bundled() {
            return false;
        }
        unsafe {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(title));
            content.setBody(&NSString::from_str(body));
            let id = format!(
                "motrix-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &NSString::from_str(&id),
                &content,
                None,
            );
            center.addNotificationRequest_withCompletionHandler(&request, None);
        }
        true
    }
}

/// Request permission up front (macOS only; no-op elsewhere).
pub fn init() {
    #[cfg(target_os = "macos")]
    macos::request_permission();
}

pub fn send(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        if macos::notify(title, body) {
            return;
        }
        // Not running from a bundle: AppleScript fallback.
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            body.replace('\\', "").replace('"', "'"),
            title.replace('"', "'"),
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let title = title.to_string();
        let body = body.to_string();
        std::thread::spawn(move || {
            let _ = notify_rust::Notification::new()
                .appname("Motrix")
                .summary(&title)
                .body(&body)
                .show();
        });
    }
}
