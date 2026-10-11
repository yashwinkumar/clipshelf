# Contributing

Clipshelf is early. Open an issue before starting large changes.

## Setup

See the requirements and commands in [README.md](README.md). Run `npm install`, then `npm test` and `npm run dev`.

## Layout and rules

Read [ARCHITECTURE.md](ARCHITECTURE.md) first. Short version:

- Operating system calls go in `src-tauri/src/platform/` only.
- Put decision logic in pure functions with unit tests (see `capture/filter.rs` and `capture/debounce.rs`).
- Keep `main.rs` to setup. Keep Tauri commands thin.
- Writes to the system clipboard go through `platform::write_text` and must be acknowledged to the poller (see ARCHITECTURE.md). Do not write to the clipboard anywhere else.
- SQL lives only in `storage/`. Bind every value as a parameter. Build SQL text only from constants.
- The interface is plain HTML, CSS and JavaScript. Keep presentation out of Rust and logic out of the page.

## Privacy rules for every change

- Never log or print clipboard content. Display it only in the intended history interface, using textContent. Do not add `Debug` output that prints it.
- Do not commit databases, logs, recordings, screenshots with real clipboard content, or any personal data.
- Use made up samples in tests and documentation. The helper script in `scripts/` writes synthetic items.
- Do not add network access without a discussion and an update to PRIVACY.md.
- Error messages and logs must not include clipboard text or search text. Storage errors use fixed `Display` text.
- To test with an empty history, delete `~/Library/Application Support/dev.promptlyashwin.clipshelf/dev/` while the app is stopped. Never delete a release database to test.

## Checks before a pull request

```
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## Third party material

Do not add code, binaries or assets taken from other applications. Document new dependencies and their licences in the pull request.

## Licence

Contributions are licensed under the project's [MIT licence](LICENSE).


## Visual changes

- Change colours, sizes and radii in `src/css/tokens.css`. Do not hard-code them in component files.
- Keep component files small: `card.js` builds cards, `app.js` wires events, `bridge.js` is the only Tauri access.
- Do not add third-party images, icons or fonts that are not licensed for redistribution. Source-app icons come from the user's own installed apps at run time.
- Check layout at two display sizes and with long text (for example, a copy of several thousand characters).

## Local bundle builds

Run `npm run bundle:debug` from the application directory for a local debug app bundle, or `npm run bundle` for a release bundle. The optional bundle configuration enables only app packaging and does not introduce a frontend build step. Debug and release have separate data paths. A debug bundle has been built locally; the release command still requires verification. Do not publish unsigned binaries or claim Intel support has been tested on Apple silicon.

Capture pause changes must preserve the shared poll lock, discard pending debounce state and rebase the current clipboard counter on both transitions. Queued main-thread callbacks must check pause before reading clipboard content. Do not turn generic read failures into claims of denied permission without platform evidence.
