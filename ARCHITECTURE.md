# Architecture

Clipshelf has a Rust core and a plain HTML, CSS and JavaScript interface inside a Tauri window. There is no frontend build step.

## Modules

```
src/                       interface (presentation only)
  index.html  css/  js/
src-tauri/src/
  main.rs                  application setup only
  platform/                the only code that calls operating system APIs
    mod.rs  macos.rs  unsupported.rs
  capture/                 poll, debounce, filter, read, store
    mod.rs                 the capture loop and event type
    debounce.rs            change detection and delay (pure, tested)
    filter.rs              exclusion rules (pure, tested)
    state.rs               latest text and counters, text redacted from Debug
  storage/                 local history; the only code that uses SQLite
    mod.rs                 Store, HistoryState, StoreError, migration
    history.rs             insert, list, search, cap, dedupe (tested)
    paths.rs               development and release database locations (tested)
  commands.rs              Tauri commands, thin: capture_status, list_history, search_history,
                           copy_history_entry, hide_panel, app_icon
  panel/
    mod.rs                 show on the active display, hide (main thread for show)
    geometry.rs            panel rectangle and AppKit to Tauri conversion (pure, tested)
  tray.rs                  menu bar item: Show panel, Pause/resume, Quit
```

`shortcuts.rs` loads the configured global shortcut and registers it with a default fallback.

Frontend (plain ES modules, no build step):

```
src/
  index.html               structure only
  css/tokens.css           colours, sizes, radii (change values here)
  css/panel.css            window frame, toolbar, search, toast
  css/cards.css            card strip and card states
  js/bridge.js             the only code that touches the Tauri globals
  js/format.js             previews, character labels, relative times (pure)
  js/icons.js              source-app icon cache (fetched through app_icon)
  js/card.js               builds one card; textContent only
  js/feedback.js           short status message
  js/app.js                search, rows, copy feedback, window events
```

## Panel placement (milestone 5)

- `platform::active_screen` returns the visible frame (menu bar and Dock excluded) of the display under the pointer, plus the main display height.
- `panel::geometry::panel_frame` places a full width strip along the bottom, inset 8 pt, 332 pt tall. The height shrinks on short displays, never below 120 pt.
- AppKit's bottom left origin is converted to Tauri's top left origin with the main display height.
- The window is transparent with no decorations, always on top, hidden from the Dock and task switcher. `macOSPrivateApi` is required for transparency in Tauri.

## Source-app icons

`app_icon` looks up the installed app by bundle identifier and returns its icon as a PNG data URL. It runs AppKit work on the main thread. Unknown or uninstalled apps show the first letter of the app name.

## Visual rules

- Header tint comes from a fixed palette keyed by the source bundle identifier. This is Clipshelf's rule, not the reference app's.
- Previews show the first 2000 characters. Storage keeps the full text, and copy back uses it.
- The character count is always the full length.

## Data flow (milestone 3)

1. A thread samples the pasteboard change count every 250 ms.
2. A change starts a 300 ms wait. A further change restarts it, so a burst gives one read.
3. On the main thread: read type names, check them against the excluded list. If one matches, stop. No content is read.
4. Otherwise read plain text, drop empty or oversized text.
5. Accepted text is written to the history database unless it equals the newest entry. Rows older than the newest 500 are deleted in the same transaction.
6. Emit a `clipboard-event` with metadata only: counts, outcome, reason, source app label, storage result. The event type has no field for text.
7. The history view calls `search_history`, which returns text only to the interface, rendered as text, never as HTML.

## Copy back and self write suppression (milestone 4)

1. `copy_history_entry(id)` loads the entry, then takes the poll lock (`capture::PollGuard`).
2. `platform::write_text` clears the clipboard and writes plain text. It returns the change count the write produced and the count read right after.
3. `Debouncer::acknowledge_write(produced, current)` moves the stored count to the new value only if the two match, and drops any pending wait. The poller takes the same lock for each sample, so it cannot sample between the write and the acknowledgement.
4. If the counts differ, another app wrote in between. Nothing is suppressed and the poller captures that change normally.
5. A later copy by another app produces a different count, so it is captured.
6. `process_change` also skips a change whose count is no longer current when it runs, because the poller reports the newer state.

Copy back does not add a history row. No key is simulated; the user presses Cmd+V.

Limit: if another app copies at the instant before our write, that copy is overwritten and not captured.

## Storage rules

- All values are bound parameters. SQL text is built only from constants.
- Search input is escaped (`%`, `_`, `\`) and matched with `LIKE ... ESCAPE`, so it is literal.
- Errors use fixed messages. Log lines never include text or search input.
- Schema version is stored in `PRAGMA user_version` (currently 1).
- The database file is created with owner only permissions (mode 0600).

## Design rules

- Clipboard content never appears in logs, events or error messages. It appears only in history rows, in the interface.
- `platform/` is the only place that touches AppKit. Other modules take plain values.
- `main.rs` wires modules together and holds no behaviour.

## Choices and provenance

Behaviour taken from analysis of a commercial clipboard manager (Paste 7.0.2) with the REA tool:
- Poll interval 250 ms and delay 300 ms.
- Change detection by comparing the pasteboard change count with a stored value.
- Reading the frontmost app after the delay, not at the moment of copy.
- The list of six marker types.

Our own decisions, not copied:
- Skipping a copy that carries a marker type (the original's action on these types was not established).
- Text only, plain SQLite, a 100 KB limit, a 500 entry cap, and skipping an identical copy of the newest entry.
- Panel size, placement, card sizes and colours: chosen to match observed proportions. Not a copy of any asset, font or icon.
- No synthetic key press. Copy back will write to the clipboard and the user presses Command V.

No code, binaries or assets from the commercial app are used.

## Known limits

- Source app is best effort. The lookup sometimes returns nothing.
- Development builds are not app bundles, so they have no bundle identifier. Their source label can name Clipshelf itself. Test source labels on a release bundle.
- Item level type lists show non UTI types as `dyn.` identifiers. The capture code therefore checks both the pasteboard level and item level lists.
- Clipboard access prompts are controlled by macOS. See [PRIVACY.md](PRIVACY.md).

## Keyboard handling

`src/js/app.js` owns navigation and shortcut dispatch, including focus preservation across history refreshes and scrolling the selected card into view. Search-field editing retains native cursor movement and text-copy behaviour. Enter refreshes current search results before copying. The search form never submits or reloads the webview.

`src-tauri/src/shortcuts.rs` owns the Tauri global-shortcut plugin and local settings. The handler dispatches panel operations onto the main thread because placement reads AppKit screen information. Shortcut registration errors leave the tray available.

## Launch and capture hardening

The single-instance plugin is registered before other plugins. Duplicate launches and macOS reopen events show the existing panel. Capture pause is stored in CaptureState and coordinated with PollGuard. Both pause transitions rebase the detector; queued capture callbacks also check the paused state. The frontend and tray share the same capture state. A text read failure is distinct from a non-text item, and storage failures reach the panel without content logging.

## Clear-history flow

The frontend requests confirmation in a dialog with Cancel focused initially. The clear_history command requires a confirmation flag, acquires PollGuard, deletes stored entries through Store, clears the latest in-memory text, advances the capture generation and rebases the current change count. Capture callbacks compare their queued generation before reading, so callbacks from before a clear or pause transition cannot restore old entries. A history-cleared event invalidates pending frontend requests, clears the search and reloads history. Clear does not change the native clipboard or pause state.
