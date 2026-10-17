// Windows helper; runs natively or in the SAME Wine/Proton bottle as the game.
// The only remote execution performed is loading our version-checked runtime.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <wchar.h>
#include "protocol.h"

static GrimShared *shared;
static HANDLE game_handle;
static volatile LONG running=1;
static volatile LONG64 parent_heartbeat;
static CRITICAL_SECTION output_lock;
static void json_string(const char *s){
    putchar('"');for(const unsigned char *p=(const unsigned char*)s;*p;p++){
        if(*p=='"'||*p=='\\'){putchar('\\');putchar(*p);}
        else if(*p<32)printf("\\u%04x",*p);else putchar(*p);
    }putchar('"');
}
static void error(const char *message){EnterCriticalSection(&output_lock);printf("{\"error\":\"%s\",\"state\":\"error\",\"code\":%lu}\n",message,GetLastError());fflush(stdout);LeaveCriticalSection(&output_lock);}
static HMODULE remote_module(DWORD pid,const wchar_t *name){
    HANDLE snap=CreateToolhelp32Snapshot(TH32CS_SNAPMODULE|TH32CS_SNAPMODULE32,pid);if(snap==INVALID_HANDLE_VALUE)return NULL;
    MODULEENTRY32W m={0};m.dwSize=sizeof(m);HMODULE out=NULL;
    if(Module32FirstW(snap,&m))do{if(!_wcsicmp(m.szModule,name)){out=m.hModule;break;}}while(Module32NextW(snap,&m));CloseHandle(snap);return out;
}
static DWORD find_game(void){
    HANDLE snap=CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0);if(snap==INVALID_HANDLE_VALUE)return 0;
    PROCESSENTRY32W p={0};p.dwSize=sizeof(p);DWORD found=0;unsigned count=0;
    if(Process32FirstW(snap,&p))do{
        if((!_wcsicmp(p.szExeFile,L"Grim Dawn.exe")||!_wcsicmp(p.szExeFile,L"GrimDawn.exe"))&&remote_module(p.th32ProcessID,L"Game.dll")){found=p.th32ProcessID;count++;}
    }while(Process32NextW(snap,&p));CloseHandle(snap);
    if(count>1){SetLastError(ERROR_MORE_DATA);error("Multiple games are running in this bottle; close the extra game");return 0;}return found;
}
static BOOL inject(DWORD pid,const wchar_t *dll){
    HMODULE local=NULL;
    FARPROC load=(FARPROC)LoadLibraryW;
    if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,(LPCWSTR)load,&local))return FALSE;
    wchar_t path[MAX_PATH];if(!GetModuleFileNameW(local,path,MAX_PATH))return FALSE;
    wchar_t *base=wcsrchr(path,L'\\');base=base?base+1:path;
    HMODULE remote=remote_module(pid,base);if(!remote)return FALSE;
    void *remote_load=(char*)remote+((char*)load-(char*)local);
    SIZE_T length=(wcslen(dll)+1)*sizeof(wchar_t),written=0;
    void *memory=VirtualAllocEx(game_handle,NULL,length,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);if(!memory)return FALSE;
    BOOL ok=FALSE;
    if(WriteProcessMemory(game_handle,memory,dll,length,&written)&&written==length){
        HANDLE thread=CreateRemoteThread(game_handle,NULL,0,(LPTHREAD_START_ROUTINE)remote_load,memory,0,NULL);
        if(thread){if(WaitForSingleObject(thread,15000)==WAIT_OBJECT_0){DWORD code=0;ok=GetExitCodeThread(thread,&code)&&code!=0;}else{CloseHandle(thread);return FALSE;}CloseHandle(thread);}
    }
    VirtualFreeEx(game_handle,memory,0,MEM_RELEASE);return ok;
}
static void stop_session(void){if(shared){InterlockedExchange(&shared->connected,0);InterlockedExchange64(&shared->heartbeat,0);}}
static DWORD WINAPI monitor(LPVOID unused){
    (void)unused;
    while(InterlockedCompareExchange(&running,0,0)){
        ULONGLONG now=GetTickCount64();
        if(WaitForSingleObject(game_handle,0)==WAIT_OBJECT_0){stop_session();error("Game closed; modifiers reset");InterlockedExchange(&running,0);ExitProcess(0);}
        if(now-(ULONGLONG)InterlockedCompareExchange64(&parent_heartbeat,0,0)>GRIM_LEASE_MS){stop_session();InterlockedExchange(&running,0);ExitProcess(0);}
        InterlockedExchange64(&shared->heartbeat,(LONG64)now);
        const char *state=shared->state==GRIM_READY?"ready":shared->state==GRIM_ERROR?"error":shared->state==GRIM_INACTIVE?"inactive":"connecting";
        EnterCriticalSection(&output_lock);
        printf("{\"characterName\":");json_string(shared->player_name);
        printf(",\"playerId\":\"");for(int i=0;i<16;i++)printf("%02x",shared->player_save_id[i]);
        printf("\",\"difficulty\":%ld,\"portalTotal\":%ld,\"portalKnown\":[%ld,%ld,%ld],\"portalsAdded\":%ld,\"portalSaveRequested\":%s",shared->difficulty,shared->portal_total,shared->portal_known[0],shared->portal_known[1],shared->portal_known[2],shared->portals_added,shared->portal_save_requested?"true":"false");
        printf(",\"state\":\"%s\",\"capabilities\":%ld,\"frames\":%lld,\"equipmentBroken\":%lld,\"equipmentUsable\":%lld,\"protectedRolls\":%lld,\"equipmentRolls\":%lld,\"equipmentCreated\":%lld,\"equipmentHits\":%lld,\"lastEquipmentBase\":%.7g,\"lastEquipmentResult\":%.7g,\"skillCount\":%u,\"lastLootBase\":%.7g,\"lastLootResult\":%.7g,\"playerCookie\":\"%llu\",\"points\":{\"skill\":%ld,\"attribute\":%ld,\"devotion\":%ld},\"totalDevotion\":%ld,\"xpHits\":%lld,\"lootHits\":%lld,\"densityHits\":%lld,\"cooldownHits\":%lld,\"lastXpBase\":%ld,\"lastXpResult\":%ld,\"lastDensityBase\":%ld,\"lastDensityResult\":%ld,\"lastCooldownBaseMs\":%ld,\"lastCooldownResultMs\":%ld,\"acknowledged\":%ld,\"commandResult\":%ld,\"error\":\"%s\"}\n",state,shared->capabilities,shared->frames,shared->equipment_broken,shared->equipment_usable,shared->protected_rolls,shared->equipment_rolls,shared->equipment_created,shared->equipment_hits,(double)shared->last_equipment_base,(double)shared->last_equipment_result,shared->reserved,(double)shared->last_loot_base,(double)shared->last_loot_result,(unsigned long long)shared->player_cookie,shared->skill,shared->attribute,shared->devotion,shared->total_devotion,shared->xp_hits,shared->loot_hits,shared->density_hits,shared->cooldown_hits,shared->last_xp_base,shared->last_xp_result,shared->last_density_base,shared->last_density_result,shared->last_cooldown_base_ms,shared->last_cooldown_result_ms,shared->acknowledged,shared->command_result,shared->error);
        fflush(stdout);LeaveCriticalSection(&output_lock);Sleep(250);
    }return 0;
}
int wmain(int argc,wchar_t **argv){
    InitializeCriticalSection(&output_lock);setvbuf(stdout,NULL,_IONBF,0);
    DWORD pid=find_game();if(!pid){error("No 64-bit Grim Dawn found; use the same Wine or CrossOver bottle as the game");return 2;}
    if(argc==2&&!wcscmp(argv[1],L"--probe")){printf("{\"state\":\"found\",\"pid\":%lu,\"gameModule\":\"%p\"}\n",pid,remote_module(pid,L"Game.dll"));return 0;}
    if(argc!=3||wcscmp(argv[1],L"--attach")){error("Expected --probe or --attach with the runtime DLL path");return 2;}
    if(GetFileAttributesW(argv[2])==INVALID_FILE_ATTRIBUTES){error("Runtime DLL not found");return 2;}
    game_handle=OpenProcess(PROCESS_QUERY_INFORMATION|PROCESS_CREATE_THREAD|PROCESS_VM_OPERATION|PROCESS_VM_WRITE|PROCESS_VM_READ|SYNCHRONIZE,FALSE,pid);
    if(!game_handle){error("Cannot open game process");return 2;}
    wchar_t name[96];
    for(unsigned version=2;version<GRIM_VERSION;version++){
        wsprintfW(name,L"Local\\GrimTrainerSession-v%u-%lu",version,pid);
        HANDLE old=OpenFileMappingW(FILE_MAP_READ,FALSE,name);if(!old)continue;
        GrimShared *view=MapViewOfFile(old,FILE_MAP_READ,0,0,68);
        // Protocols 2-6 place state after their 24-byte settings at offset 56.
        BOOL busy=!view||(view->connected&&GetTickCount64()-(ULONGLONG)view->heartbeat<GRIM_LEASE_MS)||*(volatile LONG*)((unsigned char*)view+(version>=7?60:56))!=GRIM_INACTIVE;
        if(view)UnmapViewOfFile(view);CloseHandle(old);
        if(busy){error("An older trainer is still connected; close it before using this version");return 2;}
    }
    grim_mapping_name(name,pid);
    HANDLE mapping=CreateFileMappingW(INVALID_HANDLE_VALUE,NULL,PAGE_READWRITE,0,sizeof(GrimShared),name);BOOL existing=GetLastError()==ERROR_ALREADY_EXISTS;
    if(!mapping){error("Cannot create session mapping");return 2;}
    shared=(GrimShared*)MapViewOfFile(mapping,FILE_MAP_ALL_ACCESS,0,0,sizeof(GrimShared));if(!shared){error("Cannot map session state");return 2;}
    if(existing&&shared->connected&&GetTickCount64()-(ULONGLONG)shared->heartbeat<GRIM_LEASE_MS){error("Another trainer is already connected");return 2;}
    if(!existing){ZeroMemory(shared,sizeof(*shared));shared->magic=GRIM_MAGIC;shared->version=GRIM_VERSION;}
    if(shared->magic!=GRIM_MAGIC||shared->version!=GRIM_VERSION){error("Incompatible runtime already attached; restart the game");return 2;}
    shared->controller_pid=GetCurrentProcessId();shared->settings=(GrimSettings){1,1,1,0,0,0,1};shared->command=shared->acknowledged;shared->error[0]=0;
    InterlockedExchange(&shared->connected,1);InterlockedExchange64(&shared->heartbeat,(LONG64)GetTickCount64());InterlockedExchange64(&parent_heartbeat,(LONG64)GetTickCount64());
    if(!remote_module(pid,L"grim-session-v8.dll")&&!inject(pid,argv[2])){stop_session();error("Could not load runtime into the game");return 2;}
    HANDLE watcher=CreateThread(NULL,0,monitor,NULL,0,NULL);if(!watcher){stop_session();return 2;}
    char line[512];
    while(fgets(line,sizeof(line),stdin)&&running){
        if(!strncmp(line,"ping",4)){InterlockedExchange64(&parent_heartbeat,(LONG64)GetTickCount64());continue;}
        if(!strncmp(line,"stop",4))break;
        if(!strncmp(line,"set ",4)){
            GrimSettings s={0};int used=0;
            if(sscanf(line+4,"%f %f %f %f %ld %ld %ld %n",&s.xp,&s.drop,&s.density,&s.cooldown,&s.zero_cooldown,&s.enabled,&s.prevent_broken,&used)!=7||!isfinite(s.xp)||!isfinite(s.drop)||!isfinite(s.density)||!isfinite(s.cooldown)||s.xp<1||s.xp>10||s.drop<1||s.drop>10||s.density<1||s.density>5||s.cooldown<0||s.cooldown>90||(s.zero_cooldown!=0&&s.zero_cooldown!=1)||(s.enabled!=0&&s.enabled!=1)||(s.prevent_broken!=0&&s.prevent_broken!=1)){error("Invalid session settings");continue;}
            InterlockedIncrement(&shared->sequence);shared->settings=s;MemoryBarrier();InterlockedIncrement(&shared->sequence);
        }else if(!strncmp(line,"grant ",6)){
            long seq,a,b,c,cap;unsigned long long player;
            if(sscanf(line+6,"%ld %llu %ld %ld %ld %ld",&seq,&player,&a,&b,&c,&cap)!=6||seq<=shared->acknowledged||a<0||b<0||c<0||a>1000||b>1000||c>1000||cap<1||cap>10000){error("Invalid point grant");continue;}
            if(shared->command!=shared->acknowledged){error("A point grant is still pending");continue;}
            shared->command_kind=1;shared->grant_skill=a;shared->grant_attribute=b;shared->grant_devotion=c;shared->devotion_cap=cap;shared->expected_player=(LONG64)player;MemoryBarrier();InterlockedExchange(&shared->command,seq);
        }else if(!strncmp(line,"portals ",8)){
            long seq,difficulty;unsigned long long player;char uid[33]={0};
            if(sscanf(line+8,"%ld %llu %ld %32s",&seq,&player,&difficulty,uid)!=4||seq<=shared->acknowledged||difficulty<0||difficulty>2||strlen(uid)!=32){error("Invalid portal request");continue;}
            unsigned char bytes[16];BOOL valid=TRUE;
            for(int i=0;i<16;i++){
                char pair[3]={uid[i*2],uid[i*2+1],0};char *end=NULL;unsigned long v=strtoul(pair,&end,16);
                if(end!=pair+2){valid=FALSE;break;}bytes[i]=(unsigned char)v;
            }
            if(!valid){error("Invalid character identity");continue;}
            if(shared->command!=shared->acknowledged){error("A character action is still pending");continue;}
            shared->command_kind=2;shared->expected_player=(LONG64)player;shared->expected_difficulty=difficulty;
            memcpy(shared->expected_save_id,bytes,16);shared->portals_added=0;
            MemoryBarrier();InterlockedExchange(&shared->command,seq);
        }else error("Unknown runtime command");
    }
    stop_session();InterlockedExchange(&running,0);WaitForSingleObject(watcher,1000);
    for(int i=0;i<40&&shared->state!=GRIM_INACTIVE&&shared->state!=GRIM_ERROR;i++)Sleep(50);
    printf("{\"state\":\"disconnected\",\"restored\":%s}\n",shared->state==GRIM_INACTIVE?"true":"false");
    UnmapViewOfFile(shared);CloseHandle(mapping);CloseHandle(game_handle);CloseHandle(watcher);return 0;
}
