// Exercise the production hook callbacks in an isolated Windows process.
// No game process, game archive, character save or injected DLL is used.
#include "../session.c"
#include <assert.h>
static GrimShared control;
static int actor,skill_object,other_actor;
static void *actor_now=&actor;
static uint32_t skills=5,attributes=4,devotions=0,total=0;
static void *skill_vector_data[1012]={&skill_object};
static void *skill_vector[3]={skill_vector_data,skill_vector_data+1012,skill_vector_data+1012};
static float observed_loot;
static unsigned char captured_protection;
static uint32_t fake_class(void *self,cpp_bool base){(void)self;(void)base;return 0;}
static void fake_no_broken(void *self,cpp_bool value){((unsigned char*)self)[0x50]=1;((unsigned char*)self)[0x51]=value;}
static void fake_loot_name(void *self,void *a,void *b,void *c,void *d){(void)a;(void)b;(void)c;(void)d;captured_protection=((unsigned char*)self)[0x50];}
static const char *actor_name="records/creatures/enemies/test_normal.dbr";
static uint32_t fake_xp(void *a,uint32_t b,uint32_t c,uint32_t d,const void *e){(void)a;(void)b;(void)c;(void)d;(void)e;return 42;}
static float fake_cd(void *a,cpp_bool *b,cpp_bool c){(void)a;(void)b;(void)c;return 10.f;}
static float fake_float(void *a,const void *b,float c){(void)a;(void)b;return c;}
static float fake_float_string(void *a,const void *b,float c){return hook_float_c(a,msvc_string(b),c);}
static const char *fake_name(void *a){(void)a;return actor_name;}
static void fake_loot(void *a,void *b){(void)b;observed_loot=hook_float_c(a,"chanceToEquipMisc1",10.f);}
static void *fake_equipment(void *self,const void *table,const char *key,int location,uint32_t *seed,void *random){(void)self;(void)location;(void)seed;(void)random;observed_loot=hook_float_c((void*)table,key,10.f);return &skill_object;}
static void *fake_main(void *a){(void)a;return actor_now;}
static void *fake_skills(void *a){(void)a;return skill_vector;}
static uint32_t get_s(void *a){(void)a;return skills;}
static uint32_t get_a(void *a){(void)a;return attributes;}
static uint32_t get_d(void *a){(void)a;return devotions;}
static uint32_t get_t(void *a){(void)a;return total;}
static void add_s(void *a,uint32_t n){(void)a;skills+=n;}
static void add_a(void *a,uint32_t n){(void)a;attributes+=n;}
static void add_d(void *a,uint32_t n){(void)a;devotions+=n;}
static void add_t(void *a,uint32_t n){(void)a;total+=n;}
#include "portal_fixtures.h"
int main(void){
    install_portal_fakes();
    shared=&control;shared->connected=1;shared->heartbeat=(LONG64)GetTickCount64();shared->settings=(GrimSettings){2,3,2,50,0,1};
    original_xp=fake_xp;original_cooldown=fake_cd;original_float_c=fake_float;original_float_s=fake_float_string;original_loot=fake_loot;original_equipment=fake_equipment;get_name=fake_name;get_item_class=fake_class;set_no_broken=fake_no_broken;original_loot_name=fake_loot_name;
    get_main=fake_main;get_skills=fake_skills;get_skill=get_s;get_attribute=get_a;get_devotion=get_d;get_total=get_t;add_skill=add_s;add_attribute=add_a;add_devotion=add_d;add_total=add_t;
    tls_loot=TlsAlloc();tls_float=TlsAlloc();assert(tls_loot!=TLS_OUT_OF_INDEXES&&tls_float!=TLS_OUT_OF_INDEXES);
    sample_player(NULL);assert(shared->player_cookie!=0);assert(player_skill_count==1012);
    assert(hook_xp(NULL,0,0,0,NULL)==84);
    cpp_bool flag=0;assert(hook_cooldown(&skill_object,&flag,0)==5.f);assert(hook_cooldown(&other_actor,&flag,0)==10.f);
    shared->settings.zero_cooldown=1;assert(hook_cooldown(&skill_object,&flag,0)==0.f);shared->settings.zero_cooldown=0;
    assert(hook_float_c(NULL,"chanceToEquipMisc1",10.f)==10.f);
    hook_loot(NULL,NULL);assert(observed_loot==30.f);assert(shared->last_loot_base==10.f&&shared->last_loot_result==30.f);
    TlsSetValue(tls_loot,(void*)1);assert(hook_float_c(NULL,"chanceToEquipMisc3",0.f)==0.f);TlsSetValue(tls_loot,NULL);assert(shared->last_loot_base==10.f&&shared->last_loot_result==30.f);
    TlsSetValue(tls_loot,(void*)1);assert(hook_float_c(NULL,"chanceToEquipMisc2",60.f)==100.f);assert(hook_float_c(NULL,"chanceToEquipHead",10.f)==10.f);
    struct {const char *p;uint64_t padding;size_t length,capacity;} string={"chanceToEquipMisc1",0,18,18};
    assert(hook_float_s(NULL,&string,10.f)==30.f);TlsSetValue(tls_loot,NULL);
    assert(hook_equipment(&actor,NULL,"chanceToEquipChest",0,NULL,NULL)==&skill_object);assert(observed_loot==30.f);assert(shared->equipment_hits==1&&shared->equipment_created==1);assert(shared->last_equipment_base==10.f&&shared->last_equipment_result==30.f);
    actor_name="records/creatures/pc/malepc01.dbr";hook_equipment(&actor,NULL,"chanceToEquipChest",0,NULL,NULL);assert(observed_loot==10.f&&shared->equipment_hits==1);
    actor_name="records/creatures/enemies_npc/vendor.dbr";hook_equipment(&actor,NULL,"chanceToEquipChest",0,NULL,NULL);assert(observed_loot==10.f&&shared->equipment_hits==1);
    actor_name="records/creatures/npcs/vendor.dbr";hook_equipment(&actor,NULL,"chanceToEquipChest",0,NULL,NULL);assert(observed_loot==10.f&&shared->equipment_hits==1);
    actor_name="records/creatures/enemies/test_normal.dbr";hook_equipment(&actor,NULL,"chanceToEquipChestItem1",0,NULL,NULL);assert(observed_loot==10.f&&shared->equipment_hits==1);
    assert(TlsGetValue(tls_loot)==NULL);assert(hook_float_c(NULL,"chanceToEquipChest",10.f)==10.f);
    unsigned char loader[96]={0};loader[0x51]=1;shared->settings.prevent_broken=1;
    TlsSetValue(tls_loot,(void*)2);hook_loot_name(loader,NULL,NULL,NULL,NULL);assert(captured_protection==1&&shared->protected_rolls==1);assert(loader[0x50]==0&&loader[0x51]==1);
    TlsSetValue(tls_loot,(void*)1);hook_loot_name(loader,NULL,NULL,NULL,NULL);assert(captured_protection==0&&shared->protected_rolls==1);
    TlsSetValue(tls_loot,NULL);hook_loot_name(loader,NULL,NULL,NULL,NULL);assert(captured_protection==0);
    TlsSetValue(tls_loot,(void*)2);shared->settings.prevent_broken=0;hook_loot_name(loader,NULL,NULL,NULL,NULL);assert(captured_protection==0);
    shared->settings.prevent_broken=1;shared->settings.drop=1;hook_loot_name(loader,NULL,NULL,NULL,NULL);assert(captured_protection==0);
    shared->settings.drop=3;shared->connected=0;hook_loot_name(loader,NULL,NULL,NULL,NULL);assert(captured_protection==0&&loader[0x50]==0);shared->connected=1;TlsSetValue(tls_loot,NULL);
    shared->expected_player=player_epoch;shared->grant_skill=3;shared->grant_attribute=2;shared->grant_devotion=1;shared->devotion_cap=55;shared->command_kind=1;shared->command=1;
    sample_player(NULL);assert(shared->acknowledged==1&&shared->command_result==0);assert(skills==8&&attributes==6&&devotions==1&&total==1);
    sample_player(NULL);assert(skills==8&&total==1);
    actor_now=&other_actor;shared->command=2;sample_player(NULL);assert(shared->acknowledged==2&&shared->command_result==1&&skills==8);
    shared->connected=0;hook_equipment(&actor,NULL,"chanceToEquipChest",0,NULL,NULL);assert(observed_loot==10.f);assert(hook_xp(NULL,0,0,0,NULL)==42);assert(hook_cooldown(&skill_object,&flag,0)==10.f);hook_loot(NULL,NULL);assert(observed_loot==10.f);assert(skills==8&&total==1);
    shared->connected=1;shared->heartbeat=(LONG64)(GetTickCount64()-3000);assert(hook_xp(NULL,0,0,0,NULL)==42);assert(hook_cooldown(&skill_object,&flag,0)==10.f);
    test_portals();
    puts("PASS: one-time current-difficulty portals, deduplication, identity and difficulty guards, save request,  XP scaling, cooldown ownership and zero mode, loot scope/cap/no-double-scale, point grant/deduplication/character-change guard, disconnect and expired-lease originals, retained progress.");return 0;
}
