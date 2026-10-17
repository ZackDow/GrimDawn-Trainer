# Grim Dawn Trainer

A local desktop app for Windows, Linux, and macOS. Create gameplay profiles and edit unspent character points with previews, automatic backups, and restore.

**v0.1 uses gameplay database overrides and offline save editing. Live memory toggles are not included.**

## Start here

On the development machine, the Linux build is in `release/`. Install the `.deb` with your package manager, or run the included executable on a system with the required GTK/WebKit libraries.

1. Open **Game files**. The app detects Steam libraries, Proton saves, and CrossOver bottles. Use **Browse** for a GOG/custom installation or a save folder it did not find.
2. In **Gameplay**, set your multipliers and select **Preview gameplay changes**. Review the affected records and choose **Back up & apply** with Grim Dawn closed.
3. Add `/basemods` to Grim Dawn's Steam launch options, preserving any existing arguments. Start a new game session. The generated archive is installed at `Grim Dawn/mods/database.arz`.
4. In **Character**, select a save, set unspent balances, and preview the changes. Close Grim Dawn before applying. **For a Steam cloud save, also close Steam.** Restart Steam after the write completes and keep the newly edited local file if Steam presents a sync conflict.
5. Use **Backups** to preview a restore. **Disable installed rules** removes this app's gameplay override after backing it up; it does not reverse character-point edits.

The browser opened by `npm run dev` is explicitly an interface preview. Filesystem operations work in the native desktop app started by `npm run desktop` or in a packaged build.

## Controls

| Control                         | What v0.1 changes                                                                                                                                                                                    |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| XP, 1–10×                       | Monster-kill experience formulas for the game's difficulty variants. Quest rewards and the level curve stay unchanged.                                                                               |
| Drop chance, 1–10×              | Existing miscellaneous loot-slot probabilities on monsters, capped at 100%. Slots that may award direct quest items are excluded. Equipped combat gear is not changed.                               |
| Epic/legendary weighting, 1–10× | Existing mixed monster loot-slot choices that lead to epic/legendary tables. Level gates, table fallbacks, and individual item odds still apply.                                                     |
| Monster density, 1–5×           | Eligible ordinary pool minimums, maximums, and ordinary entry limits. Known boss/quest/event/totem pools, champion counts, and single-spawn pools are excluded. Individual counts are capped at 250. |
| Cooldown reduction, 0–90%       | Base cooldown fields on player mastery active skills. Gear/transmuter modifiers, animations, energy costs, devotion/item procs, and pet subskills retain their own rules.                            |
| Zero cooldown                   | An experimental override for the same active-skill records.                                                                                                                                          |
| Skill / attribute points        | Unspent balances, 0–10,000. Previously allocated skills and attributes are preserved.                                                                                                                |
| Devotion points                 | Unspent balance and its matching total. The installed game's spending limit and constellation requirements are preserved.                                                                            |

Multipliers are always generated from the original base/expansion databases, so repeatedly installing a 2× profile does not compound it into 4×. Expansion archives override earlier records in base → GDX1 → GDX2 → GDX3 order.

## Existing mods and game updates

The app does not overwrite an unknown `mods/database.arz`. If one is present, the preview offers export for manual merging. Arbitrary custom mods, Crucible, and other custom-game databases are outside this release's integration scope.

A game update may change record definitions. Rebuild the profile after updating. Source hashes are checked again between preview and apply, so an update during that interval cancels the write. Unknown archive/save layouts are refused rather than guessed.

The supported save layout is header 2, data version 8, with the block versions described in [FORMATS.md](docs/FORMATS.md). In particular, v0.1 refuses non-empty item-skill binding payloads until their current layout has independent fixtures. This does not prevent normal gameplay profiles from working.

## Backups and local storage

Application data lives in the platform's local data directory under `GrimDawnTrainer`. The exact path appears in **Backups**. Each operation records the target, before/after SHA-256 hashes, and the before/after file bytes. Backups are not uploaded or automatically deleted.

Before applying, the app checks running processes, locks against concurrent trainer writes, verifies the preview's source hashes, backs up and verifies the original, and uses a temporary file plus atomic replacement. A restore is itself backed up. Only `player.gdc` is edited for point changes; other character files are left intact.

`GRIM_TRAINER_DATA_DIR` can select a separate application-data directory for testing.

## Build from source

Install Node.js 22.12+ and stable Rust, plus the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm ci
npm run desktop
```

Package on the corresponding OS:

```sh
# Linux
npm run package -- --bundles deb,appimage
# Windows: Visual Studio C++ Build Tools and WebView2 required
npm run package -- --bundles nsis,msi
# macOS: Xcode command-line tools required
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run package -- --target universal-apple-darwin --bundles dmg
```

`.github/workflows/build.yml` builds these packages on their respective runners and uploads artifacts. It does not publish releases. Windows and macOS binaries need their own build/runtime verification; a successful Linux build is not evidence that those platforms passed. Public distribution signing/notarization is not configured.

The development scripts also detect a project-local Rust installation in `.tools/`, when present. That private toolchain is not included in source releases.

## Validation

```sh
npm run check
npm run build
npm run test:core
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Real-file tests are opt-in and read-only against their inputs:

```sh
GRIM_TEST_SAVE='/path/to/player.gdc' GRIM_TEST_GAME='/path/to/Grim Dawn' npm run test:core
```

See [VALIDATION.md](docs/VALIDATION.md) for the actual checked release evidence and outstanding runtime checks. Game archives, character saves, extracted records, and backups are excluded from source packages.

## Command-line tools

```sh
cargo run -p grim-core --bin grimctl -- scan
cargo run -p grim-core --bin grimctl -- inspect-save '/path/to/player.gdc'
cargo run -p grim-core --bin grimctl -- build-rules '/path/to/Grim Dawn' profile.json output.arz
```

`build-rules` writes a verified archive plus a manifest to a new output path; it does not install it. `edit-copy` writes a new save copy and requires explicit point values and a devotion cap; use `--help` for its arguments. Existing output files are never overwritten by these commands.

## Structure

- `src/`: React desktop interface and typed IPC calls.
- `src-tauri/`: native window, dialogs, and serialized background commands.
- `crates/grim-core/`: native ARZ/GDC codecs, game discovery, rule generation, point edits, backups, and CLI.
- `docs/`: format provenance and validation boundaries.

This is an independent utility and is not affiliated with Crate Entertainment. No game assets or executable modifications are distributed with it.
