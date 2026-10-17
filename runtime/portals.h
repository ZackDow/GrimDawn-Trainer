#ifndef GRIM_PORTALS_H
#define GRIM_PORTALS_H
#define GRIM_MAX_PORTALS 512

typedef struct { unsigned char bytes[16]; } PortalId;
typedef void *(*PortalGlobalFn)(void);
typedef void *(*PortalPointerFn)(void*);
typedef void *(*PortalIndexFn)(void*,int);
typedef int (*PortalCountFn)(void*);
typedef const char *(*PortalTypeFn)(void*);
typedef void (*PortalLockFn)(void*);
typedef unsigned char (*PortalKnownFn)(void*,const PortalId*,int);
typedef void (*PortalAddFn)(void*,const PortalId*,int);
typedef const wchar_t *(*PlayerNameFn)(void*);
typedef unsigned char (*PortalSaveFn)(void*);
static struct {
    PortalGlobalFn manager;
    PortalCountFn group_count, id_count, difficulty;
    PortalIndexFn group, id;
    PortalTypeFn type;
    PortalLockFn lock, unlock;
    PortalPointerFn game_info, save_id;
    PortalKnownFn known;
    PortalAddFn add;
    PlayerNameFn name;
    PortalSaveFn save;
    void **engine;
} portal_api;
static BOOL readable(const void *ptr,size_t size);
static BOOL portal_symbol(HMODULE module,const char *name,void **out){
    *out=(void*)GetProcAddress(module,name);return *out!=NULL;
}
static BOOL portal_bind(HMODULE game,HMODULE engine){
    BOOL ok=TRUE;
#define PORTAL_BIND(module,symbol,field) ok=ok&&portal_symbol(module,symbol,(void**)&portal_api.field)
    PORTAL_BIND(engine,"?Get@InstanceGroupManager@GAME@@SAPEAV12@XZ",manager);
    PORTAL_BIND(engine,"?GetNumGroups@InstanceGroupManager@GAME@@QEBAHXZ",group_count);
    PORTAL_BIND(engine,"?GetGroup@InstanceGroupManager@GAME@@QEAAPEAVInstanceGroup@2@H@Z",group);
    PORTAL_BIND(engine,"?GetType@InstanceGroup@GAME@@QEBAPEBDXZ",type);
    PORTAL_BIND(engine,"?GetNumID@InstanceGroup@GAME@@QEAAHXZ",id_count);
    PORTAL_BIND(engine,"?GetID@InstanceGroup@GAME@@QEAAAEBVUniqueId@2@H@Z",id);
    PORTAL_BIND(engine,"?LockGroupList@InstanceGroupManager@GAME@@QEAAXXZ",lock);
    PORTAL_BIND(engine,"?UnlockGroupList@InstanceGroupManager@GAME@@QEAAXXZ",unlock);
    PORTAL_BIND(engine,"?GetGameInfo@Engine@GAME@@QEAAPEAVGameInfo@2@XZ",game_info);
    PORTAL_BIND(engine,"?GetDifficulty@GameInfo@GAME@@QEBAIXZ",difficulty);
    PORTAL_BIND(game,"?IsTeleportUIDKnown@Player@GAME@@QEBA_NAEBVUniqueId@2@W4GameDifficulty@2@@Z",known);
    PORTAL_BIND(game,"?AddTeleportUID@Player@GAME@@QEAAXAEBVUniqueId@2@W4GameDifficulty@2@@Z",add);
    PORTAL_BIND(game,"?SaveGame@GameEngine@GAME@@QEAA_NXZ",save);
    PORTAL_BIND(game,"?GetPlayerName@Player@GAME@@QEBAPEBGXZ",name);
    PORTAL_BIND(game,"?GetUniqueSaveId@Player@GAME@@QEBAAEBVUniqueId@2@XZ",save_id);
    portal_api.engine=(void**)GetProcAddress(engine,"?gEngine@GAME@@3PEAVEngine@1@EA");
#undef PORTAL_BIND
    return ok&&portal_api.engine!=NULL;
}
static int portal_difficulty(void){
    if(!portal_api.engine||!*portal_api.engine)return -1;
    void *info=portal_api.game_info(*portal_api.engine);
    int difficulty=info?portal_api.difficulty(info):-1;
    return difficulty>=0&&difficulty<=2?difficulty:-1;
}
// The same Riftgates instance groups used by ItemDifficultyUnlock::Use.
// Read through the game's API under its group-list lock; no foreign C++ allocation.
static BOOL portal_collect(PortalId out[GRIM_MAX_PORTALS],size_t *count){
    *count=0;void *manager=portal_api.manager();if(!manager)return FALSE;
    portal_api.lock(manager);BOOL ok=TRUE;
    int groups=portal_api.group_count(manager);
    if(groups<0||groups>100000)ok=FALSE;
    for(int g=0;ok&&g<groups;g++){
        void *group=portal_api.group(manager,g);if(!group)continue;
        const char *type=portal_api.type(group);
        if(!type||strcmp(type,"Riftgates"))continue;
        int ids=portal_api.id_count(group);
        if(ids<0||ids>GRIM_MAX_PORTALS){ok=FALSE;break;}
        for(int i=0;ok&&i<ids;i++){
            const PortalId *id=portal_api.id(group,i);
            if(!readable(id,sizeof(*id))){ok=FALSE;break;}
            PortalId zero={0};if(!memcmp(id,&zero,sizeof(zero)))continue;
            BOOL duplicate=FALSE;
            for(size_t j=0;j<*count;j++)if(!memcmp(&out[j],id,sizeof(*id))){duplicate=TRUE;break;}
            if(duplicate)continue;
            if(*count==GRIM_MAX_PORTALS){ok=FALSE;break;}
            out[(*count)++]=*id;
        }
    }
    portal_api.unlock(manager);return ok&&*count>0;
}
static void portal_counts(void *player,const PortalId *ids,size_t count,LONG known[3]){
    for(int d=0;d<3;d++){
        known[d]=0;
        for(size_t i=0;i<count;i++)if(portal_api.known(player,&ids[i],d))known[d]++;
    }
}
static int portal_apply(void *player,int difficulty,const PortalId *ids,size_t count,LONG *added){
    *added=0;if(difficulty!=portal_difficulty())return 4;
    LONG before[3],after[3];portal_counts(player,ids,count,before);
    for(size_t i=0;i<count;i++)if(!portal_api.known(player,&ids[i],difficulty)){
        portal_api.add(player,&ids[i],difficulty);(*added)++;
    }
    portal_counts(player,ids,count,after);
    for(int d=0;d<3;d++)if(after[d]!=(d==difficulty?(LONG)count:before[d]))return 6;
    return 0;
}
#endif
