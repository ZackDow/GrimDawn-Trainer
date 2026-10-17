# v0.2.1 validation

Checked locally on 14 September 2026. This update adds **Prevent broken equipment** and corrects the distinction between generated equipment and actual ground drops.

## Current-session diagnosis

A read-only observer confirmed the user's live trainer was enabled with XP 10×, loot 10×, density 5× and 90% cooldown reduction. The running runtime had recorded 6,666 modified equipment chances, including 20% → 100%. This confirms the equipment generation multiplier was active; it does not establish a matching count of useful drops.

The installed zombie torso loot table contains `brokenOnly = 100`. Game code separately rejects broken equipment in its death-drop path. An attempted live audit of that particular path did not account for the session's equipment drops, so its counters are not used as a drop-rate measurement. All three temporary audit hooks were restored and the user's trainer settings were preserved.

## Change and checks

- Prevent broken equipment invokes the game's own `LootLoader::SetNoBrokenItems` before enemy equipment generation and restores the temporary loader flags afterward. It only applies with live modifiers enabled, loot above 1× and the checkbox selected. It does not modify existing inventory or game files.
- Equipment telemetry now separates broken and non-broken results and remains visible when a normal equipment chance is already 100%. It is explicitly generation telemetry, not a count of items on the ground.
- The bridge refuses to install over an older active runtime, including the short period while its hooks are resetting.
- Production callback tests cover enabled, disabled, original-loot, non-enemy scope, disconnected scope and loader-flag restoration, plus the existing multiplier and point-grant protections.
- Rust tests and Clippy, TypeScript and packaging checks are recorded in the local build outputs.
- Live no-broken comparison in the active game: 94 of the first 100 enemy equipment results contained a broken affix; 6 did not. After enabling the game’s no-broken mode, all 319 sampled results were non-broken. This is a sequential generation sample across new spawns, not a controlled count of picked-up or ground items. The temporary hook then confirmed restoration.
- The user’s other running multipliers remained unchanged throughout the comparison.
- The integrated revision 7 runtime passed an 11-function byte check: original before attachment, patched during attachment, original again after killing the helper and waiting for its watchdog. The packaged Rust transport connected at normal settings, maintained its heartbeat without UI polling, and confirmed restoration on disconnect.

## Existing verified behaviour

[The v0.2.0 validation](VALIDATION-v0.2.0.md) records the XP, ordinary spawn, miscellaneous loot and cooldown calculation checks, native transport checks and hook-restoration tests. Its equipment-created count includes broken gear and must not be treated as useful loot or ground-drop counts.

The runtime is restricted to Steam x64 build 24825149 with exact Game.dll and Engine.dll hashes. Linux/Proton is the locally exercised platform. Native Windows/macOS installers and CrossOver gameplay remain unverified. No real point grants were sent during these checks. Earned gameplay progress is retained.
