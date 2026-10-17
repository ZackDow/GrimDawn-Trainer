#ifndef GRIM_PROTOCOL_H
#define GRIM_PROTOCOL_H
#include <windows.h>
#include <stdint.h>
#define GRIM_MAGIC 0x47525432u
#define GRIM_VERSION 8u
#define GRIM_LEASE_MS 2000u
enum { GRIM_LOADING=0, GRIM_READY=1, GRIM_INACTIVE=2, GRIM_ERROR=3 };
typedef struct {
    float xp, drop, density, cooldown;
    LONG zero_cooldown, enabled, prevent_broken;
} GrimSettings;
typedef struct {
    uint32_t magic, version;
    volatile LONG sequence, connected;
    volatile LONG64 heartbeat;
    uint32_t controller_pid, reserved;
    GrimSettings settings;
    volatile LONG state, capabilities;
    volatile LONG64 frames, xp_hits, loot_hits, density_hits, cooldown_hits;
    volatile LONG64 player_cookie;
    volatile LONG skill, attribute, devotion, total_devotion;
    volatile LONG last_xp_base,last_xp_result,last_density_base,last_density_result;
    volatile LONG last_cooldown_base_ms,last_cooldown_result_ms;
    volatile LONG command, acknowledged, command_result;
    LONG grant_skill,grant_attribute,grant_devotion,devotion_cap;
    LONG64 expected_player;
    volatile float last_loot_base,last_loot_result;
    volatile LONG64 equipment_rolls,equipment_created,equipment_hits;
    volatile float last_equipment_base,last_equipment_result;
    volatile LONG64 equipment_broken,equipment_usable,protected_rolls;
    LONG command_kind, expected_difficulty;
    unsigned char expected_save_id[16], player_save_id[16];
    volatile LONG difficulty, portal_total, portal_known[3], portals_added, portal_save_requested;
    char player_name[192];
    char error[256];
} GrimShared;
static void grim_mapping_name(wchar_t *out,DWORD pid){wsprintfW(out,L"Local\\GrimTrainerSession-v8-%lu",pid);}
#endif
