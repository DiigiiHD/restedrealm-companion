# RestedRealm Companion app

The Windows app described in [docs/COMPANION-APP.md](../docs/COMPANION-APP.md). It is being built in phases; this folder holds what exists so far.

## `core/`: phase 1, done

The Rust core that the app will run on. It ports the Python companion in [`companion/`](../companion/) and matches it byte for byte, so it can take over an existing queue and pairing without re-uploading anything:

- **Same queue.** It opens the same `queue.sqlite3` with the same schema.
- **Same digests.** Every record is canonicalised exactly as Python's `json.dumps(sort_keys=True, ensure_ascii=False)` writes it, including Python's float spelling. Rust's own shortest float formatting differs from Python for some 17-digit values, so `canon::python_float` rounds explicitly.
- **Same credential.** It reads and writes the Windows Credential Manager entry `RestedRealmCollector/restedrealm.com` in the same JSON format.

| Module | What it does |
| --- | --- |
| `lua.rs` | Reads SavedVariables as data only. It never runs Lua. |
| `canon.rs` | Canonical JSON and SHA-256 digests. |
| `save.rs` | Settled-file reads, transactional import, validation, and freeing the addon's space with a backup. |
| `queue.rs` | The local SQLite queue, status, settings and forgetting. |
| `upload.rs` | Prose redaction, batching, HTTPS without redirects, pairing, and acknowledging records only after a complete answer. |
| `credentials.rs` | Windows Credential Manager, plus an in-memory store for tests. |
| `game.rs` | Finding each account's save, and whether WoW is running. |

`rrc-core` is a command-line tool over the same code for development and support. Run `cargo run` in `app/` with no arguments to see its commands. Players will use the app.

## `src-tauri/` and `ui/`: phase 2, built

The Windows app around the core. It has not yet run on a Windows PC; everything below was built and checked on Linux, and the Windows build compiles.

| Part | What it does |
| --- | --- |
| `ui/` | The window: plain HTML, CSS and JavaScript in the website's colours, fonts (Inter and Lora, SIL Open Font License, bundled) and emblem. No build step and nothing loaded from the internet. |
| `src-tauri/src/main.rs` | Starts the app: one copy at a time, `--background` when Windows starts it, closing the window hides it next to the clock. |
| `src-tauri/src/worker.rs` | Watches the addon's save, imports it, uploads when automatic upload is on, and frees the addon's space once 200 or more records are safely queued and WoW is closed. A five-minute check catches anything the file watcher misses. |
| `src-tauri/src/commands.rs` | Everything the window may ask for. Each command does one fixed thing; the window cannot open arbitrary URLs or files. |
| `src-tauri/src/tray.rs` | The icon next to the clock: status, Upload now, Open restedrealm.com, Quit. |
| `src-tauri/src/addon.rs` | Finds the Forever folder in the usual places and installs the bundled addon. Phase 4 replaces the guesses with Battle.net discovery. |
| `src-tauri/src/labels.rs` | Plain words for record kinds. |

The app keeps its data in `%LOCALAPPDATA%\RestedRealm Companion`. On first start it copies the Python pilot's queue and backups from `%LOCALAPPDATA%\RestedRealmCollector\Companion` into that folder, once, and leaves the old folder untouched. The pairing carries over because both use the same Windows Credential Manager entry.

Automatic upload is on only after the player leaves it on in setup. Deleting local data switches it off again until the player turns it back on.

To run it during development:

```bash
cd app
npm install
npm run dev      # or: cargo run -p restedrealm-companion
```

## Checks

```bash
cd app
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# The window's screens, with a stand-in for the Rust side (see tests/ui-screens.mjs):
node ../tests/ui-screens.mjs /tmp/companion-shots

# Byte-for-byte agreement with the Python companion on generated saves:
cargo build
python ../tests/parity_check.py --rounds 400 --seed 1
```

The parity check writes random saves covering Unicode, escapes, floats of every magnitude, sparse and number-keyed tables, prose to redact and deliberately damaged files. Both implementations must accept or refuse the same saves and store identical digests, payloads and upload batches. The GitHub Actions workflow in [`docs/ci/core.yml`](../docs/ci/core.yml) runs all of this on Windows and Linux for every change once it is moved to `.github/workflows/core.yml`. It is parked in `docs/ci/` because the tool that pushed it may not create workflow files; the owner moves it with one commit on GitHub.

A known, harmless difference: Python accepts whole numbers larger than 64 bits in a save and non-ASCII digits outside strings. The addon writes neither, and the Rust core refuses them.
