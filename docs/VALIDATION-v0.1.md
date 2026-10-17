# v0.1 validation

Checked locally on 14 September 2026. This separates file-format evidence from in-game behaviour and from other operating systems.

## Passed

- TypeScript checking and optimized React/Vite build.
- Rust formatting and workspace Clippy, with warnings treated as errors.
- Nine Rust integration tests, including the two opt-in tests against the installed game and a real character save.
- Complete decode and byte-for-byte replay of a current character save (header 2, data 8, inventory/stash blocks 11).
- Point edits on a separate test copy; restoring its original point values reproduces the original encrypted bytes exactly.
- An independent current-layout reader reparsed the original and edited copies without warnings. Its only reported differences were attribute points, skill points, devotion points, and the matching devotion total.
- Native archive generation from Steam build **24825149**, merging base, GDX1, GDX2, and GDX3.
- Grim Dawn's installed **ArchiveTool.exe**, running in an isolated project-local Wine prefix, extracted the final generated archive into **2,304 actual DBR files**. Record count matched the manifest. Sample XP, cooldown, and spawn values matched the requested changes.
- Ordinary/boss/totem/proc scope boundaries, probability capping, rare-table relative weighting, and preserving unrelated values were exercised with synthetic fixtures.
- Backup, restore, disable, stale-file rejection, source-drift rejection, base-mod conflict refusal, and export were exercised through real filesystem transactions in temporary directories.
- A process with a Wine-style `Grim Dawn.exe` command line was detected as a running game.
- Linux release executable and Debian package built successfully. The native application stayed running during a 12-second startup check without a startup error. All its linked libraries resolved on this host.
- Browser UI review covered multiplier changes, cooldown feedback, navigation, character empty state, and game-file settings. The browser was explicitly in interface-preview mode.
- The original character save retained its initial SHA-256. Source game archives retained their build fingerprints. No trainer profile was installed into the actual game.

## Combined-profile fixture

Input: XP 2×, loot chance 2×, high-rarity table weighting 2×, ordinary density 2×, cooldown reduction 50%.

| Category    | Changed fields |
| ----------- | -------------: |
| XP          |              4 |
| Loot chance |          2,889 |
| Rarity      |          1,842 |
| Density     |            995 |
| Cooldown    |             53 |
| Total       |          5,783 |

The generated archive contained 2,304 records and 12,152,930 bytes. SHA-256:

`366121768df256771965c27f7e6deb103f704fd7d7cfa5beff17feb90b66e559`

The fixture is private test output, not a bundled mod. Users build profiles from their own installed game data.

## Still unverified

- Actual gameplay with the generated archive, including measured drop frequencies, density/performance under combat, cooldown interactions, and character loading after an edit. ArchiveTool acceptance is evidence of a readable game archive; it is not an in-game playtest.
- Native Windows and macOS builds and runtime behaviour. The source and CI workflow target them, but those runners were not available in this local task. No Windows installer or macOS DMG is claimed as built.
- Complete native GUI interaction through Tauri IPC. Native startup and core service transactions were checked; interactive visual QA used the browser frontend.
- Other save block versions, non-empty item-skill binding payloads, arbitrary custom mods, and non-campaign game modes.
- Live-memory trainer toggles, signing/notarization, and public distribution.

The Linux binary was linked against glibc symbols up to **2.39**. Its Debian package declares that minimum. GTK 3 and WebKitGTK 4.1 runtime libraries are required.
