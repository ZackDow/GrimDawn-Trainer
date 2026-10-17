// In-process session hooks. Game files and saved characters are never patched.
// Hook ABIs and the two spawn call sites are gated to the verified module hashes.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <bcrypt.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include "protocol.h"
#include "portals.h"
#include "vendor/minhook/include/MinHook.h"

typedef unsigned char cpp_bool;
typedef void (*UpdateFn)(void*,int);
typedef void *(*PointerFn)(void*);
typedef uint32_t (*UIntFn)(void*);
typedef void (*AddFn)(void*,uint32_t);
typedef const char *(*NameFn)(void*);
typedef uint32_t (*XpFn)(void*,uint32_t,uint32_t,uint32_t,const void*);
typedef float (*CooldownFn)(void*,cpp_bool*,cpp_bool);
typedef void (*LootFn)(void*,void*);
typedef void *(*EquipmentFn)(void*,const void*,const char*,int,uint32_t*,void*);
typedef void (*LootNameFn)(void*,void*,void*,void*,void*);
typedef void (*NoBrokenFn)(void*,cpp_bool);
typedef uint32_t (*ClassFn)(void*,cpp_bool);
typedef float (*FloatFn)(void*,const void*,float);
typedef int (*EquationFn)(void*,const void*,int);
static UpdateFn original_update;
static XpFn original_xp;
static CooldownFn original_cooldown;
static LootFn original_loot;
static EquipmentFn original_equipment;
static LootNameFn original_loot_name;
static NoBrokenFn set_no_broken;
static ClassFn get_item_class;
static FloatFn original_float_c,original_float_s,original_base_float_c,original_base_float_s;
static EquationFn original_equation;
static PointerFn get_main,get_skills;
static UIntFn get_skill,get_attribute,get_devotion,get_total;
static AddFn add_skill,add_attribute,add_devotion,add_total;
static NameFn get_name;
static GrimShared *shared;
static HMODULE game_module,engine_module;
static DWORD tls_loot,tls_float;
static SRWLOCK cache_lock=SRWLOCK_INIT;
static void *player_pointer;
static void *player_skills[4096];
static size_t player_skill_count;
static LONG64 player_epoch;
static DWORD64 last_frame_sample;
static DWORD64 last_portal_sample;

static BOOL readable(const void *ptr,size_t size){
    uintptr_t p=(uintptr_t)ptr,end=p+size;if(p<65536||end<p)return FALSE;
    while(p<end){MEMORY_BASIC_INFORMATION m;if(!VirtualQuery((void*)p,&m,sizeof(m))||m.State!=MEM_COMMIT||(m.Protect&(PAGE_GUARD|PAGE_NOACCESS)))return FALSE;uintptr_t next=(uintptr_t)m.BaseAddress+m.RegionSize;if(next<=p)return FALSE;p=next;}return TRUE;
}
static const char *msvc_string(const void *object){
    if(!readable(object,32))return NULL;
    const size_t *words=(const size_t*)object;size_t n=words[2],cap=words[3];
    if(n>1024||cap<n)return NULL;
    const char *p=cap>=16?*(const char*const*)object:(const char*)object;
    if(!readable(p,n+1)||p[n]!=0)return NULL;return p;
}
static BOOL protected_name(const char *name){
    const char *words[]={"boss","hero","quest","nemesis","bounty","special","event","endless","survival","totem","ambush","backup","test"};
    if(!name)return TRUE;for(size_t i=0;i<sizeof(words)/sizeof(words[0]);i++)if(strstr(name,words[i]))return TRUE;return FALSE;
}
static BOOL lease(void){
    return shared&&shared->connected&&GetTickCount64()-(ULONGLONG)InterlockedCompareExchange64(&shared->heartbeat,0,0)<GRIM_LEASE_MS;
}
static BOOL settings(GrimSettings *s){
    if(!lease())return FALSE;
    for(int i=0;i<4;i++){LONG a=InterlockedCompareExchange(&shared->sequence,0,0);if(a&1)continue;*s=shared->settings;MemoryBarrier();if(a==InterlockedCompareExchange(&shared->sequence,0,0))return s->enabled!=0;}
    return FALSE;
}
static BOOL local_skill(void *skill){BOOL found=FALSE;AcquireSRWLockShared(&cache_lock);for(size_t i=0;i<player_skill_count;i++)if(player_skills[i]==skill){found=TRUE;break;}ReleaseSRWLockShared(&cache_lock);return found;}
static void sample_player(void *engine){
    void *player=get_main(engine);
    AcquireSRWLockExclusive(&cache_lock);
    if(player_pointer!=player){player_pointer=player;player_epoch++;}
    player_skill_count=0;
    if(player){
        void **v=(void**)get_skills(player);
        if(readable(v,24)){
            uintptr_t begin=(uintptr_t)v[0],end=(uintptr_t)v[1];size_t count=end>=begin?(end-begin)/sizeof(void*):4097;
            if(count<=4096&&(end-begin)%sizeof(void*)==0&&readable((void*)begin,count*sizeof(void*))){memcpy(player_skills,(void*)begin,count*sizeof(void*));player_skill_count=count;shared->reserved=(uint32_t)count;}
        }
    }
    ReleaseSRWLockExclusive(&cache_lock);
    InterlockedExchange64(&shared->player_cookie,player?player_epoch:0);
    if(!player){shared->difficulty=-1;shared->portal_total=0;shared->player_name[0]=0;return;}
    shared->skill=(LONG)get_skill(player);shared->attribute=(LONG)get_attribute(player);shared->devotion=(LONG)get_devotion(player);shared->total_devotion=(LONG)get_total(player);
    shared->difficulty=portal_difficulty();
    const void *save_id=portal_api.save_id(player);
    if(readable(save_id,16))memcpy(shared->player_save_id,save_id,16);
    else memset(shared->player_save_id,0,16);
    const wchar_t *name=portal_api.name(player);shared->player_name[0]=0;
    if(name)WideCharToMultiByte(CP_UTF8,0,name,-1,shared->player_name,sizeof(shared->player_name),NULL,NULL);
    if(GetTickCount64()-last_portal_sample>=1000){
        last_portal_sample=GetTickCount64();PortalId ids[GRIM_MAX_PORTALS];size_t count=0;LONG known[3]={0};
        if(portal_collect(ids,&count))portal_counts(player,ids,count,known);
        else count=0;
        shared->portal_total=(LONG)count;for(int d=0;d<3;d++)shared->portal_known[d]=known[d];
    }
    LONG command=InterlockedCompareExchange(&shared->command,0,0);
    if(command==shared->acknowledged||!lease())return;
    if(shared->expected_player!=player_epoch){shared->command_result=1;}
    else if(shared->command_kind==2){
        shared->portals_added=0;shared->portal_save_requested=0;
        if(memcmp(shared->expected_save_id,shared->player_save_id,16))shared->command_result=1;
        else if(shared->expected_difficulty!=shared->difficulty)shared->command_result=4;
        else{
            PortalId ids[GRIM_MAX_PORTALS];size_t count=0;LONG added=0;
            if(!portal_collect(ids,&count))shared->command_result=5;
            else{
                shared->command_result=portal_apply(player,shared->expected_difficulty,ids,count,&added);
                shared->portals_added=added;shared->portal_total=(LONG)count;
                LONG known[3];portal_counts(player,ids,count,known);
                for(int d=0;d<3;d++)shared->portal_known[d]=known[d];
                if(shared->command_result==0&&added>0)shared->portal_save_requested=portal_api.save(engine);
            }
        }
    }
    else if(shared->command_kind!=1){shared->command_result=3;}
    else if(shared->grant_skill<0||shared->grant_attribute<0||shared->grant_devotion<0||shared->grant_skill>1000||shared->grant_attribute>1000||shared->grant_devotion>1000||get_skill(player)+(uint32_t)shared->grant_skill>10000||get_attribute(player)+(uint32_t)shared->grant_attribute>10000||get_total(player)+(uint32_t)shared->grant_devotion>(uint32_t)shared->devotion_cap){shared->command_result=2;}
    else{
        add_skill(player,(uint32_t)shared->grant_skill);add_attribute(player,(uint32_t)shared->grant_attribute);add_devotion(player,(uint32_t)shared->grant_devotion);add_total(player,(uint32_t)shared->grant_devotion);
        shared->command_result=0;
        shared->skill=(LONG)get_skill(player);shared->attribute=(LONG)get_attribute(player);shared->devotion=(LONG)get_devotion(player);shared->total_devotion=(LONG)get_total(player);
    }
    MemoryBarrier();InterlockedExchange(&shared->acknowledged,command);
}
static void hook_update(void *self,int ticks){
    original_update(self,ticks);
    if(lease()){InterlockedIncrement64(&shared->frames);DWORD64 now=GetTickCount64();if(now-last_frame_sample>=100){last_frame_sample=now;sample_player(self);}}
}
static uint32_t hook_xp(void *self,uint32_t a,uint32_t b,uint32_t c,const void *position){
    uint32_t base=original_xp(self,a,b,c,position),result=base;GrimSettings s;
    if(settings(&s)&&s.xp>1){double scaled=(double)base*s.xp;result=(uint32_t)fmin(scaled,1000000000.);shared->last_xp_base=(LONG)base;shared->last_xp_result=(LONG)result;InterlockedIncrement64(&shared->xp_hits);}
    return result;
}
static float hook_cooldown(void *self,cpp_bool *charges,cpp_bool include){
    float base=original_cooldown(self,charges,include),result=base;GrimSettings s;
    if(settings(&s)&&local_skill(self)&&isfinite(base)&&base>0){result=s.zero_cooldown?0:base*(1-s.cooldown/100.f);shared->last_cooldown_base_ms=(LONG)fmin(base,2000000000.f);shared->last_cooldown_result_ms=(LONG)fmin(result,2000000000.f);if(result!=base)InterlockedIncrement64(&shared->cooldown_hits);}
    return result;
}
static void hook_loot(void *self,void *items){
    ULONG_PTR old=(ULONG_PTR)TlsGetValue(tls_loot);TlsSetValue(tls_loot,(LPVOID)(old|1));
    original_loot(self,items);TlsSetValue(tls_loot,(LPVOID)old);
}
static BOOL equipment_key(const char *name){
    const char *keys[]={"Head","Neck","Medal","Chest","Legs","Feet","Hands","Waist","Shoulders","LeftHand","RightHand","Finger1","Finger2"};
    if(!name||strncmp(name,"chanceToEquip",13))return FALSE;
    for(size_t i=0;i<sizeof(keys)/sizeof(keys[0]);i++)if(!strcmp(name+13,keys[i]))return TRUE;
    return FALSE;
}
static void *hook_equipment(void *self,const void *table,const char *key,int location,uint32_t *seed,void *random){
    const char *name=get_name(self);BOOL eligible=name&&!strncmp(name,"records/creatures/enemies/",26)&&equipment_key(key);
    ULONG_PTR old=(ULONG_PTR)TlsGetValue(tls_loot);if(eligible){TlsSetValue(tls_loot,(LPVOID)(old|2));InterlockedIncrement64(&shared->equipment_rolls);}
    void *item=original_equipment(self,table,key,location,seed,random);
    if(eligible){if(item){InterlockedIncrement64(&shared->equipment_created);if(get_item_class(item,1)==5)InterlockedIncrement64(&shared->equipment_broken);else InterlockedIncrement64(&shared->equipment_usable);}TlsSetValue(tls_loot,(LPVOID)old);}
    return item;
}
// LootLoader is a per-roll object. Restore its flags after generating the result;
// the game retains ownership of all output strings and item construction.
static void hook_loot_name(void *self,void *base,void *prefix,void *suffix,void *random){
    GrimSettings s;
    BOOL protect=((ULONG_PTR)TlsGetValue(tls_loot)&2)&&settings(&s)&&s.prevent_broken&&s.drop>1;
    unsigned char *flags=(unsigned char*)self+0x50;unsigned char saved0=0,saved1=0;
    if(protect){saved0=flags[0];saved1=flags[1];set_no_broken(self,1);InterlockedIncrement64(&shared->protected_rolls);}
    original_loot_name(self,base,prefix,suffix,random);
    if(protect){flags[0]=saved0;flags[1]=saved1;}
}
static float transform_float(const char *name,float value,BOOL outer){
    ULONG_PTR scope=(ULONG_PTR)TlsGetValue(tls_loot);
    BOOL misc=(scope&1)&&name&&!strncmp(name,"chanceToEquipMisc",17)&&name[17]>='1'&&name[17]<='3'&&name[18]==0;
    BOOL gear=(scope&2)&&equipment_key(name);
    GrimSettings s;
    if(outer&&(misc||gear)&&settings(&s)&&s.drop>1&&isfinite(value)&&value>=0&&value<=100){
        float result=fminf(value*s.drop,100.f);
        if(result!=value){
            if(gear){shared->last_equipment_base=value;shared->last_equipment_result=result;InterlockedIncrement64(&shared->equipment_hits);}
            else{shared->last_loot_base=value;shared->last_loot_result=result;InterlockedIncrement64(&shared->loot_hits);}
        }
        return result;
    }
    return value;
}
static float hook_float_c(void *self,const void *name,float fallback){
    ULONG_PTR depth=(ULONG_PTR)TlsGetValue(tls_float);TlsSetValue(tls_float,(LPVOID)(depth+1));float result=original_float_c(self,name,fallback);TlsSetValue(tls_float,(LPVOID)depth);result=transform_float((const char*)name,result,depth==0);return result;
}
static float hook_float_s(void *self,const void *name,float fallback){
    ULONG_PTR depth=(ULONG_PTR)TlsGetValue(tls_float);TlsSetValue(tls_float,(LPVOID)(depth+1));float result=original_float_s(self,name,fallback);TlsSetValue(tls_float,(LPVOID)depth);result=transform_float(msvc_string(name),result,depth==0);return result;
}
static float hook_base_float_c(void *self,const void *name,float fallback){
    ULONG_PTR depth=(ULONG_PTR)TlsGetValue(tls_float);TlsSetValue(tls_float,(LPVOID)(depth+1));float result=original_base_float_c(self,name,fallback);TlsSetValue(tls_float,(LPVOID)depth);result=transform_float((const char*)name,result,depth==0);return result;
}
static float hook_base_float_s(void *self,const void *name,float fallback){
    ULONG_PTR depth=(ULONG_PTR)TlsGetValue(tls_float);TlsSetValue(tls_float,(LPVOID)(depth+1));float result=original_base_float_s(self,name,fallback);TlsSetValue(tls_float,(LPVOID)depth);result=transform_float(msvc_string(name),result,depth==0);return result;
}
static int hook_equation(void *proxy,const void *equation,int input){
    uintptr_t caller=(uintptr_t)__builtin_return_address(0)-(uintptr_t)game_module;
    int base=original_equation(proxy,equation,input),result=base;GrimSettings s;
    if((caller==0x409c1a||caller==0x409c35)&&settings(&s)&&s.density>1){
        size_t offset=caller==0x409c1a?0x48:0x68;const unsigned char *pool=(const unsigned char*)equation-offset;
        const char *name=msvc_string(pool+0x10);
        if(readable(pool,0x48)&&name&&!strncmp(name,"records/proxies/",16)&&!protected_name(name)&&*(const int*)(pool+0x34)>1&&base>=0&&base<=250){result=(int)fminf(roundf(base*s.density),250.f);shared->last_density_base=base;shared->last_density_result=result;InterlockedIncrement64(&shared->density_hits);}
    }
    return result;
}
static BOOL hash_module(HMODULE module,const char *expected){
    wchar_t path[32768];if(!GetModuleFileNameW(module,path,32768))return FALSE;
    HANDLE file=CreateFileW(path,GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,0,NULL);if(file==INVALID_HANDLE_VALUE)return FALSE;
    BCRYPT_ALG_HANDLE alg=NULL;BCRYPT_HASH_HANDLE hash=NULL;UCHAR digest[32],buffer[65536];DWORD count=0;BOOL ok=FALSE;
    if(BCryptOpenAlgorithmProvider(&alg,BCRYPT_SHA256_ALGORITHM,NULL,0)>=0&&BCryptCreateHash(alg,&hash,NULL,0,NULL,0,0)>=0){
        BOOL good=TRUE;while(ReadFile(file,buffer,sizeof(buffer),&count,NULL)&&count){if(BCryptHashData(hash,buffer,count,0)<0){good=FALSE;break;}}
        if(good&&BCryptFinishHash(hash,digest,sizeof(digest),0)>=0){char hex[65];for(int i=0;i<32;i++)sprintf(hex+i*2,"%02x",digest[i]);ok=!strcmp(hex,expected);}
    }
    if(hash)BCryptDestroyHash(hash);if(alg)BCryptCloseAlgorithmProvider(alg,0);CloseHandle(file);return ok;
}
static BOOL bind(const char *name,void **out){*out=(void*)GetProcAddress(game_module,name);return *out!=NULL;}
static BOOL create(HMODULE module,const char *name,void *hook,void **original){
    void *target=(void*)GetProcAddress(module,name);if(!target)return FALSE;
    return MH_CreateHook(target,hook,original)==MH_OK;
}
static DWORD WINAPI worker(LPVOID unused){
    (void)unused;wchar_t name[96];grim_mapping_name(name,GetCurrentProcessId());
    HANDLE mapping=OpenFileMappingW(FILE_MAP_ALL_ACCESS,FALSE,name);if(!mapping)return 0;
    shared=MapViewOfFile(mapping,FILE_MAP_ALL_ACCESS,0,0,sizeof(GrimShared));if(!shared||shared->magic!=GRIM_MAGIC||shared->version!=GRIM_VERSION)return 0;
    game_module=GetModuleHandleW(L"Game.dll");engine_module=GetModuleHandleW(L"Engine.dll");
    if(!game_module||!engine_module||!hash_module(game_module,"07775a297050e84a846af1182731700614fb8b7bb41cca46b37fd24c90529387")||!hash_module(engine_module,"9f2042e1fba6c9261c5e9f1e6d60751a49be669fdc687f87d8a082c798fd1ca0")){strcpy(shared->error,"Unsupported game build; no hooks were installed");shared->state=GRIM_ERROR;return 0;}
    tls_loot=TlsAlloc();tls_float=TlsAlloc();
    BOOL ok=portal_bind(game_module,engine_module)&&tls_loot!=TLS_OUT_OF_INDEXES&&tls_float!=TLS_OUT_OF_INDEXES&&MH_Initialize()==MH_OK;
#define BIND(name,target) ok=ok&&bind(name,(void**)&target)
    BIND("?SetNoBrokenItems@LootLoader@GAME@@QEAAX_N@Z",set_no_broken);
    BIND("?GetItemClassification@Item@GAME@@UEBA?AW4ItemClassification@2@_N@Z",get_item_class);
    BIND("?GetMainPlayer@GameEngine@GAME@@QEBAPEAVPlayer@2@XZ",get_main);
    BIND("?GetSkillList@Character@GAME@@QEBAAEBV?$vector@PEAVSkill@GAME@@@mem@@XZ",get_skills);
    BIND("?GetSkillPoints@Character@GAME@@QEBA?BIXZ",get_skill);
    BIND("?GetModifierPoints@Character@GAME@@QEBA?BIXZ",get_attribute);
    BIND("?GetDevotionPoints@Character@GAME@@QEBA?BIXZ",get_devotion);
    BIND("?GetTotalDevotionPoints@Character@GAME@@QEBA?BIXZ",get_total);
    BIND("?AddSkillPoints@Character@GAME@@QEAAXI@Z",add_skill);
    BIND("?AddModifierPoints@Character@GAME@@QEAAXI@Z",add_attribute);
    BIND("?AddDevotionPoints@Character@GAME@@QEAAXI@Z",add_devotion);
    BIND("?AddTotalDevotionPoints@Character@GAME@@QEAAXI@Z",add_total);
    get_name=(NameFn)GetProcAddress(engine_module,"?GetObjectName@Object@GAME@@QEBAPEBDXZ");ok=ok&&get_name!=NULL;
#define HOOK(module,name,hook,original) ok=ok&&create(module,name,(void*)hook,(void**)&original)
    HOOK(game_module,"?Update@GameEngine@GAME@@QEAAXH@Z",hook_update,original_update);
    HOOK(game_module,"?CalculateExperienceReward@GameEngine@GAME@@QEAAIIIIAEBVWorldVec3@2@@Z",hook_xp,original_xp);
    HOOK(game_module,"?GetCooldownTime@Skill@GAME@@QEBAMAEA_N_N@Z",hook_cooldown,original_cooldown);
    HOOK(game_module,"?GetDropMiscItems@ControllerMonster@GAME@@IEAAXAEAV?$vector@UItemReplicaInfo@GAME@@@mem@@@Z",hook_loot,original_loot);
    HOOK(game_module,"?CreateItemFromLootTable@Character@GAME@@IEAAPEAVItem@2@AEBVLoadTable@2@PEBDW4Location@LootLoader@2@AEAIAEAVRandomUniform@2@@Z",hook_equipment,original_equipment);
    HOOK(game_module,"?GetLootName@LootLoader@GAME@@UEBAXAEAV?$basic_string@DU?$char_traits@D@std@@V?$allocator@D@2@@std@@00AEAVRandomUniform@2@@Z",hook_loot_name,original_loot_name);
    HOOK(engine_module,"?GetFloat@LoadTable@GAME@@UEBAMPEBDM@Z",hook_base_float_c,original_base_float_c);
    HOOK(engine_module,"?GetFloat@LoadTable@GAME@@UEBAMAEBV?$basic_string@DU?$char_traits@D@std@@V?$allocator@D@2@@std@@M@Z",hook_base_float_s,original_base_float_s);
    HOOK(engine_module,"?GetFloat@LoadTableBinary@GAME@@UEBAMPEBDM@Z",hook_float_c,original_float_c);
    HOOK(engine_module,"?GetFloat@LoadTableBinary@GAME@@UEBAMAEBV?$basic_string@DU?$char_traits@D@std@@V?$allocator@D@2@@std@@M@Z",hook_float_s,original_float_s);
    HOOK(game_module,"?RunEquation@Proxy@GAME@@QEAAHAEBV?$basic_string@DU?$char_traits@D@std@@V?$allocator@D@2@@std@@H@Z",hook_equation,original_equation);
    if(!ok){MH_Uninitialize();strcpy(shared->error,"A required game function could not be hooked; settings remain original");shared->state=GRIM_ERROR;return 0;}
    BOOL enabled=FALSE;shared->capabilities=31;
    for(;;){
        BOOL wanted=lease();
        if(wanted!=enabled){
            MH_STATUS status=wanted?MH_EnableHook(MH_ALL_HOOKS):MH_DisableHook(MH_ALL_HOOKS);
            if(status!=MH_OK){strcpy(shared->error,"Hook transition failed; restart the game to guarantee a clean runtime");shared->state=GRIM_ERROR;return 0;}
            enabled=wanted;shared->state=enabled?GRIM_READY:GRIM_INACTIVE;
        }
        if(!enabled){shared->state=GRIM_INACTIVE;shared->command=shared->acknowledged;}
        Sleep(100);
    }
}
BOOL WINAPI DllMain(HINSTANCE instance,DWORD reason,LPVOID reserved){
    (void)reserved;
    if(reason==DLL_PROCESS_ATTACH){DisableThreadLibraryCalls(instance);HANDLE thread=CreateThread(NULL,0,worker,NULL,0,NULL);if(thread)CloseHandle(thread);}
    return TRUE;
}
