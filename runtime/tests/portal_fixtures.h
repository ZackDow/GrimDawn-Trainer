static int fake_difficulty, portal_add_calls, portal_save_calls, group_lock_depth;
static unsigned char known_portals[3][8];
static PortalId fake_save_id={{17}};
static PortalId gate_ids[]={{{1}},{{2}},{{0}},{{1}}};
static PortalId more_gate_ids[]={{{2}},{{4}}};
static PortalId quest_ids[]={{{3}}};
typedef struct {const char *type;PortalId *ids;int count;} FakePortalGroup;
static FakePortalGroup fake_groups[]={
    {"Riftgates",gate_ids,4},{"Quests",quest_ids,1},{"Riftgates",more_gate_ids,2}
};
static int fake_engine;
static void *fake_engine_ptr=&fake_engine;
static void *fake_manager(void){return &fake_engine;}
static void *fake_info(void *self){return self;}
static int fake_diff(void *self){(void)self;return fake_difficulty;}
static int fake_group_count(void *self){(void)self;return 3;}
static void *fake_group(void *self,int i){(void)self;assert(i>=0&&i<3);return &fake_groups[i];}
static const char *fake_group_type(void *self){return ((FakePortalGroup*)self)->type;}
static int fake_id_count(void *self){return ((FakePortalGroup*)self)->count;}
static void *fake_gate_id(void *self,int i){return &((FakePortalGroup*)self)->ids[i];}
static void fake_group_lock(void *self){(void)self;group_lock_depth++;}
static void fake_group_unlock(void *self){(void)self;group_lock_depth--;}
static unsigned char fake_known(void *self,const PortalId *id,int difficulty){(void)self;assert(difficulty>=0&&difficulty<3);return known_portals[difficulty][id->bytes[0]];}
static void fake_add_portal(void *self,const PortalId *id,int difficulty){(void)self;assert(difficulty>=0&&difficulty<3);known_portals[difficulty][id->bytes[0]]=1;portal_add_calls++;}
static const wchar_t *fake_player_name(void *self){(void)self;return L"Runtime test";}
static void *fake_unique_save_id(void *self){(void)self;return &fake_save_id;}
static unsigned char fake_save_game(void *self){(void)self;portal_save_calls++;return 1;}
static void install_portal_fakes(void){
    portal_api.manager=fake_manager;portal_api.group_count=fake_group_count;portal_api.group=fake_group;
    portal_api.type=fake_group_type;portal_api.id_count=fake_id_count;portal_api.id=fake_gate_id;
    portal_api.lock=fake_group_lock;portal_api.unlock=fake_group_unlock;portal_api.engine=&fake_engine_ptr;
    portal_api.game_info=fake_info;portal_api.difficulty=fake_diff;portal_api.known=fake_known;
    portal_api.add=fake_add_portal;portal_api.name=fake_player_name;portal_api.save_id=fake_unique_save_id;portal_api.save=fake_save_game;
}
static void test_portals(void){
    PortalId ids[GRIM_MAX_PORTALS];size_t count=0;LONG added=-1;
    assert(portal_collect(ids,&count)&&count==3&&group_lock_depth==0);
    assert(ids[0].bytes[0]==1&&ids[1].bytes[0]==2&&ids[2].bytes[0]==4);
    known_portals[0][1]=1;known_portals[1][4]=1;known_portals[2][2]=1;
    assert(portal_apply(&actor,0,ids,count,&added)==0&&added==2&&portal_add_calls==2);
    assert(known_portals[0][2]&&known_portals[0][4]&&!known_portals[0][3]);
    assert(known_portals[1][4]&&!known_portals[1][1]&&known_portals[2][2]&&!known_portals[2][1]);
    assert(portal_apply(&actor,0,ids,count,&added)==0&&added==0&&portal_add_calls==2);
    fake_difficulty=1;assert(portal_apply(&actor,0,ids,count,&added)==4&&portal_add_calls==2);
    fake_groups[0].count=-1;assert(!portal_collect(ids,&count)&&group_lock_depth==0);fake_groups[0].count=4;
    shared->connected=1;shared->heartbeat=(LONG64)GetTickCount64();shared->settings.enabled=0;
    shared->command_kind=2;shared->expected_player=player_epoch;shared->command=3;shared->expected_difficulty=1;
    memset(shared->expected_save_id,0,16);sample_player(NULL);assert(shared->acknowledged==3&&shared->command_result==1&&portal_add_calls==2);
    memcpy(shared->expected_save_id,&fake_save_id,16);shared->expected_difficulty=0;shared->command=4;
    sample_player(NULL);assert(shared->acknowledged==4&&shared->command_result==4&&portal_add_calls==2);
    shared->expected_difficulty=1;shared->command=5;sample_player(NULL);
    assert(shared->acknowledged==5&&shared->command_result==0&&shared->portals_added==2&&portal_add_calls==4&&portal_save_calls==1);
    assert(shared->portal_known[0]==3&&shared->portal_known[1]==3&&shared->portal_known[2]==1&&shared->portal_save_requested);
    sample_player(NULL);assert(portal_add_calls==4&&portal_save_calls==1);
    shared->connected=0;shared->command=6;sample_player(NULL);assert(portal_add_calls==4&&shared->acknowledged==5);
}
