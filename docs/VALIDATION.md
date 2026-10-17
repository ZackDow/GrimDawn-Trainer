# v0.3 validation

Checked locally on 14 September 2026.

## Cleanup

The desktop has one screen with live modifiers, one-time character actions, and collapsed connection settings/calculation details. Offline editing and file-mod profile commands were removed from Tauri IPC. Save parsing, profile building and the old CLI are behind the optional `legacy-tools` Cargo feature, which is off in desktop builds. Installation discovery no longer scans or parses character saves. Existing backups and saved settings are retained.

Unused per-call atomic instrumentation was removed from the hot game hooks. The live runtime remains memory-only with an independent reset watchdog. The source archive retains optional recovery/development tools; release packages contain only the desktop executable, its helper/DLL, documentation and licenses.

## Riftgates

The implementation follows the portal-only part of the game's merit-item operation: collect the current world's `Riftgates` instance groups and call `Player::AddTeleportUID` for the current difficulty. It does not call merit-item use, quest-completion or difficulty-completion functions. It validates the connection session, player cookie, displayed character name and save ID, including fresh characters whose save ID is zero. It also validates difficulty on the game thread before changing anything.

The collected IDs are validated and deduplicated before mutation. Already known portals are skipped. After the operation, all collected IDs must be known on the selected difficulty and the other two difficulties must retain their previous counts. A successful addition requests the game's normal save operation.

Isolated production-callback tests passed for current-difficulty scope, duplicate IDs, excluded non-riftgate groups, idempotence, stale character identity, changed difficulty, invalid groups, disconnected operation, acknowledgement deduplication and the game-save request. Existing XP/loot/cooldown/point tests passed too.

Live execution on **Nexus / Normal** added 72 riftgates. The game then saved the character. An independent read of the encrypted save's teleport block (block 6, verified against the game's save writer) confirmed counts changed from `[0, 0, 0]` to `[72, 0, 0]` for Normal, Elite and Ultimate. Point balances were unchanged. A pre-action save copy and its SHA-256 were retained in the user's trainer backup folder. The temporary connection disconnected with restoration confirmed.

## Other checks

Rust default desktop and all-feature builds were checked. Legacy integration tests passed; the two real-file entries remain opt-in when their environment variables are absent. TypeScript checking and browser review cover the single-screen layout, connection disclosure and disabled portal action before connection.

Previously verified live calculations remain unchanged: XP, ordinary spawn counts, cooldown reductions and equipment probabilities. The no-broken generation test sampled 94 broken outcomes among 100 initial equipment results, then 319 results without broken affixes while the option was active. These are generation results, not ground-drop counts.

Compatibility remains limited to exact Steam x64 build 24825149 module hashes. Linux/Proton is exercised locally. Windows/macOS installers and CrossOver gameplay remain unverified. No point grants were sent during portal verification. The revision 8 runtime also passed an 11-function byte check: original before attachment, patched while connected, and original again after the helper was killed and its independent watchdog expired. Nexus retained all 72 Normal riftgates throughout that reset check.
