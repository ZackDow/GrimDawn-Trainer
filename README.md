# Grim Dawn Trainer v0.3

A small desktop trainer with live modifiers and one-time character actions. Windows, Linux/Proton and macOS/CrossOver share the Windows x64 game runtime. Linux/Proton is the platform tested locally.

## Use

1. Start 64-bit Grim Dawn and enter the playable world.
2. Open the trainer and **Connect to running game**. Installation discovery is automatic; folder and Wine/CrossOver settings are available in the collapsed connection sections.
3. Set XP, loot, monster density or cooldown, then **Apply live settings**. Further changes apply live.
4. **Disconnect & reset** or close the trainer to restore normal calculations. Earned progress stays saved. An independent watchdog restores hooks if the trainer exits unexpectedly.

Equipment and density changes affect new monsters. **Prevent broken equipment** applies when loot is above 1×; it uses the game's own no-broken-item generation mode. It does not guarantee rare items. Equipment counters distinguish non-broken and broken results; they count generation, not items on the ground.

## One-time character actions

- **Grant points** adds skill, attribute or devotion points to the loaded character. The normal devotion cap is retained.
- **Unlock all riftgates** adds missing campaign riftgates to the current character's current difficulty. The character and difficulty are displayed above the controls. The operation uses the same Riftgates groups as the game's merit items, without completing quests or other difficulties. Repeating it adds no duplicates. After a successful change, the trainer requests a normal game save.

These actions remain as character progress after the trainer closes. They never run automatically on connection or character changes.

## What's included

The app contains live controls, character actions, connection settings and collapsed calculation details. It does not include the old offline editor or file-mod profile screens. The desktop build does not parse character saves on startup or compile the optional legacy tools.

The Linux portable folder contains the application, its `runtime/` folder and documentation/licenses. Keep the runtime folder beside the executable. Start it with `sh start.sh`, which also works after a USB stick, cloud drive or zip tool drops the executable permission. GTK 3, WebKitGTK 4.1 and glibc 2.39+ are required on Linux.

The runtime accepts the inspected Steam x64 build 24825149 by exact Game.dll and Engine.dll hashes. Unknown builds and overlapping older runtimes are refused. Native Windows/macOS installers and real CrossOver operation remain unverified. See [validation](docs/VALIDATION.md).

## Upgrading

Close the older trainer before opening this version; Grim Dawn can remain open. Reconnect and reapply desired modifiers after restarting the trainer.

A v0.1 archive owned by this trainer is backed up and retired automatically when it is unused. If the game was launched with `/basemods`, restart without that option first. Existing backups and settings are preserved. Unrelated mods are not removed.

## Development

Install Node.js 22.12+, Rust 1.85+, MinGW-w64 and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Debian 13 all of them come from apt:

```sh
sudo apt install nodejs npm cargo rustc rust-clippy rustfmt gcc-mingw-w64-x86-64-posix libwebkit2gtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev
```

```sh
npm ci
npm run desktop
npm run package -- --bundles deb
# Windows: --bundles nsis,msi
# macOS: --target universal-apple-darwin --bundles dmg
```

The source includes the compiled helper and DLL. Rebuild them with MinGW-w64 using `npm run build:runtime`. Set `GRIM_MINGW_CC` if necessary. MinHook's license is included with the runtime.

```sh
npm run check
npm run test:core
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The old save/archive utilities are opt-in developer tools, excluded from the desktop build:

```sh
cargo run -p grim-core --features legacy-tools --bin grimctl -- --help
```

`npm run dev` is an interface preview; it has no access to the game. No game files or saves are distributed.
