# v0.2 validation

Checked locally on 14 September 2026. Live observations below use the user's running Steam x64 Grim Dawn under Proton on Linux. No point grants were sent to the real character.

## Gameplay observations

| Control                      | Evidence                                                                                                                          | Scope                                                                                                                                                                                                   |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| XP at 2×                     | 195 calculation events; examples 5 → 10, 6 → 12, 7 → 14                                                                           | Actual monster XP reward calculations during combat.                                                                                                                                                    |
| Ordinary density at 2×       | 120 calculation events; examples 3 → 6, 8 → 16, 9 → 18                                                                            | Actual ordinary spawn min/max calculations in fresh areas. This does not count every visible monster.                                                                                                   |
| Cooldown reduction at 50%    | 88 calculation events; examples 3 → 1.5 seconds, 6 → 3 seconds, 12 → 6 seconds; user reported faster skills                       | Player skill cache includes all 1,012 skill objects. The old 512-object ceiling prevented the cooldown modifier from applying.                                                                          |
| Miscellaneous loot at 10×    | 96 modified loot calculations across 48 monster-death calls                                                                       | The user reported that equipment output was still sparse; this counter alone does not establish more equipment drops.                                                                                   |
| Cooldown reduction at 90%    | 118 modified calculations; 3 → 0.3 seconds and 4 → 0.4 seconds                                                                    | Actual combat with the stronger setting.                                                                                                                                                                |
| Equipment probability at 10× | 324 modified chances, including 5% → 50%, 20% → 100% and 50% → 100%; 813 equipment items generated across the observed new spawns | These are successful equipment creations for monsters, not a count of equipment already dropped on the ground. Exact equipment-slot keys only; player/NPC gear and per-table item weights are excluded. |

The earlier cooldown log stored millisecond values multiplied by 1,000 in its diagnostic fields. The modifier calculation itself was correct. Revision 5 reports the correct milliseconds, and the UI displays seconds.

The final revision 6 session also recorded 140 modified miscellaneous chances across 67 death callbacks and 170 shortened cooldown calculations. Disconnect confirmed restoration.

Revision 6 retains the last changed probability for display instead of allowing a later empty slot to overwrite it with 0 → 0. It also records equipment rolls and successfully generated equipment separately.

These are direct calculation observations, not a statistical estimate of long-run item counts or rarity. Loot chance is capped at 100%; 10× chance does not guarantee 10× items.

## Session restoration

Normal test disconnects reported `restored: true`. A separate check compared the first 16 bytes of all ten hooked functions in the running game with their original DLL bytes before attachment, during attachment, and after forcibly terminating only the bridge. All were original before attachment, patched during attachment, and original again after the independent timeout. Earned progress is retained. Existing spawned monsters, generated loot, and a cooldown already started are not rolled back.

The trainer's v0.1 `mods/database.arz` was present but the actual game command line lacked `/basemods`, explaining why the file-based multiplier profile was unused. v0.2 backed up and removed that exact trainer-owned archive after verifying it was unused. It did not remove unrelated mods or change the game's executable files.

## Automated checks

- Production C callbacks exercised in an isolated Windows test process: XP scaling; cooldown ownership and zero mode; the 1,012-skill cache; loot scope, cap and double-scaling protection; enemy-only equipment creation, exact equipment keys and excluded player/NPC/lookalike paths; one-time point grants; duplicate/stale-character rejection; original calculations on disconnect and lease expiry; retained granted points.
- Rust: live protocol bounds test and nine integration test entries. Two real-file entries are opt-in and return early without their environment variables; their actual file-codec validation from v0.1 is recorded separately.
- TypeScript and optimized Vite build; Rust formatting and workspace Clippy with warnings treated as errors.
- Browser frontend review is interface-only; it cannot connect to a game. The native transport is checked separately.
- The actual Rust desktop transport detected the running game's Wine executable/prefix, staged its runtime resources, connected, maintained its native heartbeat through four seconds without UI polling, and disconnected with `restored: true`. All modifiers remained normal for this check.
- The Linux v0.2 release binary and Debian package built successfully with runtime revision 6 included.

See [v0.1 validation](VALIDATION-v0.1.md) for retained offline save/archive validation. Its old statement that no profile was installed describes that earlier checkpoint, not the later migration above.

## Compatibility boundary

Only Steam build **24825149**, x64, is currently enabled by exact SHA-256 checks:

- Game.dll: `07775a297050e84a846af1182731700614fb8b7bb41cca46b37fd24c90529387`
- Engine.dll: `9f2042e1fba6c9261c5e9f1e6d60751a49be669fdc687f87d8a082c798fd1ca0`

Unknown builds are refused before hooks are installed. Linux + the installed Steam Proton environment was tested locally. Native Windows installers, macOS installers, and real CrossOver sessions still need platform runners; no such packages or live tests are claimed here. The source includes the shared compiled Windows bridge/DLL, its C source, and the desktop build workflow for all three systems.

The Linux build requires glibc 2.39+, GTK 3 and WebKitGTK 4.1. The portable package includes the adjacent runtime folder required by the executable.
