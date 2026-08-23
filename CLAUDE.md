# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A system clipboard manager built with Tauri 2 + React 19 + TypeScript + Vite. UI is in Chinese. Migrated from a C# WPF version. **Windows-only** (uses clipboard-win/winreg; file capture/paste via CF_HDROP).

**Portable layout (XDG-style), everything next to the exe** (`lib.rs::exe_dir`): `config/` holds settings.json + storage pointer + window-state.json (via `app_config_dir()`), `data/` holds clipboard.db + images/ (via `app_data_dir_default()`); first launch migrates them from the legacy `%APPDATA%/com.lyz.clipboard-manager-tauri` dir (`migrate_legacy_layout`). Uninstalling the NSIS package deletes install-dir data.

## Build Commands

```bash
# Full app (from project root)
npm run tauri dev          # Vite + Rust with hot reload
npm run tauri build        # Production build

# Frontend only
npm run dev                # Vite dev server (port 1420)
npm run build              # tsc + vite build
npm test                   # vitest run (single pass)

# Rust only (from src-tauri/)
cargo check                # Type check
cargo clippy               # Lint
cargo test                 # Unit tests
```

Note: on this machine use the GNU toolchain (`cargo +stable-x86_64-pc-windows-gnu`); `.cargo/config.toml` already pins the mingw ld workaround. MSVC (`cl.exe`) is not installed.

## Architecture

**Two IPC mechanisms connect frontend to backend:**

1. **Tauri Commands** (`invoke()` from frontend): `activate_item`, `delete_item`, `toggle_favorite`, `pause_monitoring`, `get_image_base64`, `save_settings`, `load_settings`, `show_window`, `hide_window`, `close_app`, `register_hotkey`, `get_close_behavior`, `set_close_behavior`, `enable_win_v_integration`, `disable_win_v_integration`, `set_always_on_top`, `get_paste_mode`, `set_paste_mode`, `get_db_path`, `create_group`, `update_group`, `delete_group`, `set_item_group`

2. **Tauri Events** (`listen()` in frontend): `clipboard-changed`, `item-deleted`, `monitoring-paused`, `groups-changed`, `ask-close-behavior`, `paste-mode-changed`

**Dual SQLite connections** to the same file (`{APPDATA}/com.lyz.clipboard-manager-tauri/clipboard.db`):
- Frontend: `@tauri-apps/plugin-sql` for SELECT queries (`src/lib/db.ts`, read-only helper `queryItems`)
- Backend: `sqlx::SqlitePool` for INSERT/UPDATE/DELETE (`src-tauri/src/lib.rs`)
- WAL mode + busy_timeout=5000 set at startup to prevent lock contention

## Key Files

**Rust (`src-tauri/src/`):**
- `lib.rs` — App setup: plugins, tray, monitor, hotkey, cleanup, command registration; `--minimized` startup flag handling; programmatic main-window creation + app keep-alive (see "hide = destroy" pattern below)
- `utils/window_manager.rs` — Window lifecycle: `create_main_window` (config parity + geometry/pinned restore + close-request wiring), `destroy_main_window` (persists geometry, kills WebView2), `show_or_create` (all summon paths; background-thread rebuild with `CREATING` reentry guard)
- `services/clipboard_monitor.rs` — Polls clipboard every 500ms via arboard + clipboard-win (CF_HDROP file lists, CF_DIB fallback), SHA-256 dedup, emits events
- `services/paste_service.rs` — Clipboard write (arboard / clipboard-win CF_HDROP) + Ctrl+V simulation (enigo)
- `services/settings_service.rs` — Settings via tauri-plugin-store
- `services/tray_service.rs` — System tray menu (pause state is persisted to settings)
- `services/storage_service.rs` — Data location: `resolve_data_dir()` at startup (bootstrap `storage.json` in the default config dir may point to a custom dir), two-phase migration (write pointer → restart → cold-start copy + rewrite absolute image paths in DB), `change_storage_location`/`reset_storage_location` commands
- `services/image_cleanup.rs` — Safe deletion of item image files (only inside app images dir)
- `commands/` — All Tauri command handlers (clipboard / groups / settings / window / db_path)
- `migrations/` — SQLite schema (items + groups tables)

**React (`src/`):**
- `App.tsx` — Root: ThemeProvider + SearchBar + GroupTabs + ClipboardList + StatusBar + SettingsPanel
- `stores/clipboardStore.ts` — Zustand store (items, groups, filters, UI state, maxItems)
- `hooks/useClipboardListener.ts` — Listens for events, reloads items from DB
- `hooks/useDatabase.ts` — Initial data/settings load on mount; reloads groups on `groups-changed`
- `lib/db.ts` — Database access layer with `queryItems()` (read-only)
- `lib/queryBuilder.ts` — Dynamic SQL builder (search with LIKE ESCAPE, group/favorites filters, LIMIT from settings)

## Key Patterns

- **Hide = destroy, show = recreate**: hiding the main window destroys it entirely — the whole `msedgewebview2.exe` tree exits and memory returns to the OS. Summon paths (hotkey toggle, tray, `show_window`) all go through `window_manager::show_or_create`, which recreates the window (~1s) and restores geometry from `window-state.json` (a standalone file written live by Moved/Resized events — NOT stored in AppSettings, because the settings panel round-trips the whole settings object on every instant-save and would null unknown fields) + pinned state from settings. The window is created hidden and shown on `PageLoadEvent::Finished` to avoid a white flash. The app runs headless after the last window dies: `lib.rs` handles `RunEvent::ExitRequested { code: None }` with `api.prevent_exit()`; real exits must call `app.exit(0)` explicitly (tray quit, `close_app`, `CloseBehavior::Close`). The window is NOT declared in `tauri.conf.json` — `create_main_window` is the single creation path. Direct WebView2 API calls behind Tauri's back (manual `controller.Close()`, `TrySuspend`, `SetIsVisible`) are forbidden — wry owns the controller; that class of hack caused white screens/dead input before
- **Smart click (activate_item)**: single-clicking an item defaults to PASTE — destroy the window, restore foreground focus to the window captured when the hotkey showed the panel (`SetForegroundWindow` + poll in `utils/input_focus.rs`), then simulate Ctrl+V. Text items skip the clipboard write when the clipboard already holds identical content. "Copy only" mode is a manual toggle in the StatusBar (session-sticky); useful where a stray Ctrl+V is unwanted (e.g. Explorer). Tray-opened windows have no captured target and fall back to a fixed delay
- **Suppress mechanism**: When pasting, `ClipboardMonitor.suppress = true` for 500ms to prevent re-capturing the pasted content
- **Always-on-top**: 📌 button in the header (`set_always_on_top`), persisted as `pinned` in settings
- **Content dedup**: SHA-256 hash checked against DB; existing items only get `last_used_at` refreshed instead of a duplicate insert (copy counts are not tracked)
- **Image handling**: Saved as PNG in app data dir; `file_path` stores absolute path; `content` stores `[图片]`; deleting/cleaning items removes the image file
- **File handling**: Explorer copies captured via CF_HDROP (paths joined by `\n` in `content`); pasting writes CF_HDROP back
- **Event-driven UI**: Frontend never polls; listens for backend events and re-queries DB
- **ClipboardType enum**: 0=Text, 1=Link, 2=Image, 3=File
- **Paused state**: persisted in settings, applied to monitor at startup; tray toggle and settings panel both persist

## Configuration

- **Global hotkey**: Ctrl+Shift+V (configurable); optional Win+V integration (requires admin, HKLM registry)
- **Clipboard polling**: 500ms interval
- **Cleanup task**: Hourly, deletes expired items (retention_days) and excess (max_item_count), skips favorites, removes orphaned image files
- **Autostart**: passes `--minimized` (app starts hidden in tray)
- **Settings**: JSON file via tauri-plugin-store (not SQLite); newly added fields use `#[serde(default)]` for backward compat
- **Dark mode**: Tailwind `class` strategy, persisted in localStorage

## Common Pitfalls

- WAL mode is required for dual-connection pattern; without it, SQLITE_BUSY errors occur
- The 500ms suppress window in `paste_item` must be long enough for the paste to complete
- `enigo` requires OS accessibility permissions on macOS/Linux
- Image `file_path` uses platform-native separators; DB URL converts to forward slashes
- SQLite `datetime('now')` stores UTC; the frontend must parse it with a `Z` suffix or timestamps skew by the local offset
- LIKE queries must pair `escapeLike()` with an `ESCAPE '\'` clause (see `queryBuilder.ts`)
- clipboard-win `set_clipboard()` takes data by value and is incompatible with `FileList`'s `Setter<[T]>` impl — hold a `Clipboard` guard and call `raw::set_file_list_with(.., DoClear)` instead; arboard calls must happen with the guard dropped
- Frontend `db.ts` calls `invoke('get_db_path')` to get the absolute path — don't hardcode `sqlite:clipboard.db`
- ALL db/images paths must resolve through `storage_service::current_data_dir`/`images_dir` — never hardcode `app_config_dir`/`app_data_dir`; the user may have moved data to a custom dir. Migration only happens at cold start (no open SQLite connections); never copy `clipboard.db` while the app is running. The resolved images dir must also be granted at startup via `asset_protocol_scope().allow_directory()` — frontend thumbnails load through the asset protocol (`convertFileSrc`), whose config scope only covers the default `$APPDATA/images/*`
- `std::fs::canonicalize` on Windows returns `\\?\`-prefixed extended paths — NEVER persist them (storage.json, DB columns) or build `sqlite:` URLs from them (`sqlite://?/D:/...` crashes the backend pool at startup). Use `storage_service::strip_extended_prefix`; the backend pool must connect via `SqliteConnectOptions::filename(path)`, not a URL string
- `cargo test` binaries fail to launch in some shell environments (0xc0000139 or silent exit) even though the test profile compiles; pre-existing environment quirk — rely on `cargo check`/`clippy`, run `cargo test` from an interactive terminal

## Development Workflow

When using `/dw` command, follow this workflow:
1. Use Agent tool for ALL implementation work
2. Use Skill tool with `skill="requesting-code-review"` for code review after implementation
3. Use Skill tool with `skill="verification-before-completion"` for final verification
4. Run tasks in parallel when possible
5. NEVER implement code directly — always dispatch subagents

## Karpathy Guidelines

Behavioral guidelines to reduce common LLM coding mistakes, derived from [Andrej Karpathy's observations](https://x.com/karpathy/status/2015883857489522876) on LLM coding pitfalls.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.
