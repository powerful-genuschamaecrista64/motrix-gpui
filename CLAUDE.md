# motrix-gpui

Native macOS rewrite of the Motrix download manager (the Electron app at
`/Users/vincent/Projects/Motrix`) in Rust + GPUI + gpui-component. Replaces the
Tauri prototype in `/Users/vincent/Projects/motrix-next` (that repo's code is
not reused; its feature set is the reference).

## Run

```sh
cargo run            # needs aria2c (brew install aria2)
```

The app spawns `aria2c --enable-rpc` on port 29101 (config: `rpcPort`) with a
generated `rpcSecret`, and polls it over HTTP JSON-RPC every second.

## Layout

- `src/main.rs` — window options (transparent titlebar, traffic lights at 20,20,
  `app_owns_titlebar_drag: true`, blurred background), menus, quit-confirm.
- `src/state.rs` — `AppState` entity: config, task list, global stat, polling
  loop, task-event log, system notifications.
- `src/aria2/` — JSON-RPC client (`ureq`, blocking, background executor only),
  daemon spawner, task model.
- `src/config.rs` — `~/Library/Application Support/motrix-gpui/config.json`.
- `src/activity.rs` — daily downloaded-bytes log (`activity.json`) feeding the
  dashboard heatmap.
- `src/theme.rs` — Motrix palette overrides applied on top of gpui-component's
  theme; call `theme::set_mode` (never `Theme::change` directly) so overrides
  survive mode switches.
- `src/assets.rs` — asset source: own lucide SVGs in `assets/icons/` (AppIcon
  enum) + gpui-component's bundled icons; bundled default tracker list.
- `src/ui/` — `app.rs` (shell: chrome strip, sidebar, routes), `downloads.rs`
  (task table), `dashboard.rs`, `settings.rs` (card grid → sub-pages),
  `pages.rs` (Trackers/Plugins/Notifications), `add_task.rs`, `status_bar.rs`.

## Conventions

- Pixel values follow the UI spec extracted from the Electron renderer
  (Tailwind classes → px). Status-pill colors are fixed (same in dark mode).
- All aria2 calls go through `AppState::run_rpc` (background executor); never
  call the blocking client on the UI thread.
- uniform_list rows must set `.w_full()`; flex cells that truncate need
  `.min_w(px(0.))` + `.overflow_hidden()`.
