#![cfg(feature = "legacy-tools")]
use grim_core::{
    arz::{self, Field, Record, Value},
    discovery, gdc,
    rules::{self, GameData, Profile},
    storage,
};
use std::{fs, path::PathBuf};

fn record(name: &str, class: &str, fields: &[(&str, u16, Vec<Value>)]) -> Record {
    Record {
        name: name.into(),
        class: class.into(),
        time: 0,
        fields: fields
            .iter()
            .map(|(k, t, v)| {
                (
                    k.to_string(),
                    Field {
                        kind: *t,
                        values: v.clone(),
                    },
                )
            })
            .collect(),
    }
}
fn n(v: f64) -> Vec<Value> {
    vec![Value::Number(v)]
}
fn t(s: &str) -> Vec<Value> {
    vec![Value::Text(s.into())]
}
fn db_fixture() -> GameData {
    let mob = "records/creatures/enemies/regular.dbr";
    let low = "records/items/loottables/common.dbr";
    let high = "records/items/loottables/epic.dbr";
    let records = vec![
        record(
            "records/game/experienceformulas.dbr",
            "",
            &[
                ("experienceEquation", 2, t("a+b")),
                ("eliteExperienceEquation", 2, t("a^2")),
                ("ultimateExperienceEquation", 2, t("a+10")),
                ("challengeExperienceEquation", 2, t("a*3")),
            ],
        ),
        record(
            mob,
            "Monster",
            &[
                ("monsterClassification", 2, t("Common")),
                ("chanceToEquipMisc1", 1, n(60.)),
                ("lootMisc1Item1", 2, t(low)),
                ("chanceToEquipMisc1Item1", 0, n(900.)),
                ("lootMisc1Item2", 2, t(high)),
                ("chanceToEquipMisc1Item2", 0, n(100.)),
                ("chanceToEquipRightHand", 1, n(20.)),
            ],
        ),
        record(
            "records/proxies/pools/ordinary.dbr",
            "",
            &[
                ("templateName", 2, t("database/templates/proxypool.tpl")),
                ("spawnMin", 0, n(3.)),
                ("spawnMax", 0, n(9.)),
                ("championMax", 0, n(2.)),
                ("name1", 2, t(mob)),
                ("limit1", 0, n(1.)),
            ],
        ),
        record(
            "records/proxies/monstertotem/pools/totem.dbr",
            "",
            &[
                ("templateName", 2, t("database/templates/proxypool.tpl")),
                ("spawnMin", 0, n(3.)),
                ("spawnMax", 0, n(9.)),
                ("name1", 2, t(mob)),
            ],
        ),
        record(
            "records/skills/playerclass01/attack.dbr",
            "Skill_AttackRadius",
            &[
                (
                    "skillCooldownTime",
                    1,
                    vec![Value::Number(10.), Value::Number(8.)],
                ),
                ("skillManaCost", 1, n(20.)),
            ],
        ),
        record(
            "records/skills/playerclass01/pets/attack.dbr",
            "Skill_AttackRadius",
            &[("skillCooldownTime", 1, n(10.))],
        ),
        record(
            "records/skills/playerclass01/proc.dbr",
            "Skill_PassiveOnHit",
            &[("skillCooldownTime", 1, n(10.))],
        ),
        record(
            low,
            "LootItemTable_DynWeight",
            &[("lootName1", 2, t("records/items/common.dbr"))],
        ),
        record(
            high,
            "LootItemTable_DynWeight",
            &[("lootName1", 2, t("records/items/epic.dbr"))],
        ),
    ]
    .into_iter()
    .map(|r| (r.name.clone(), r))
    .collect();
    GameData {
        records,
        sources: Vec::new(),
        qualities: [
            ("records/items/common.dbr".into(), "Common".into()),
            ("records/items/epic.dbr".into(), "Epic".into()),
        ]
        .into(),
    }
}

#[test]
fn generated_rules_preserve_scope_and_probability_bounds() {
    let data = db_fixture();
    let p = Profile {
        name: "test".into(),
        xp: 2.,
        drop: 2.,
        rarity: 3.,
        density: 2.,
        cooldown: 50.,
        zero_cooldown: false,
    };
    let built = rules::build(&data, &p).unwrap();
    let out = arz::read(&built.bytes, |_| true).unwrap();
    let mob = &out["records/creatures/enemies/regular.dbr"];
    assert_eq!(mob.number("chanceToEquipMisc1"), Some(100.));
    assert_eq!(mob.number("chanceToEquipMisc1Item1"), Some(900.));
    assert_eq!(mob.number("chanceToEquipMisc1Item2"), Some(300.));
    assert_eq!(mob.number("chanceToEquipRightHand"), Some(20.));
    let pool = &out["records/proxies/pools/ordinary.dbr"];
    assert_eq!(pool.number("spawnMax"), Some(18.));
    assert_eq!(pool.number("championMax"), Some(2.));
    assert_eq!(pool.number("limit1"), Some(2.));
    assert!(!out
        .keys()
        .any(|n| n.contains("totem") || n.contains("/pets/") || n.ends_with("proc.dbr")));
    assert_eq!(
        out["records/skills/playerclass01/attack.dbr"].number("skillManaCost"),
        Some(20.)
    );
    assert_eq!(
        out["records/skills/playerclass01/attack.dbr"].fields["skillCooldownTime"].values,
        vec![Value::Number(5.), Value::Number(4.)]
    );
    assert_eq!(
        out["records/game/experienceformulas.dbr"].text("experienceEquation"),
        Some("(a+b)*2")
    );
}
#[test]
fn neutral_invalid_and_unsupported_profiles_do_not_build() {
    let data = db_fixture();
    assert!(rules::build(&data, &Profile::default()).is_err());
    let mut p = Profile {
        xp: f64::NAN,
        ..Profile::default()
    };
    assert!(p.validate().is_err());
    p.xp = 11.;
    assert!(p.validate().is_err());
    p.xp = 2.;
    let mut empty = db_fixture();
    empty.records.clear();
    assert!(rules::build(&empty, &p).is_err());
}
#[test]
fn archive_roundtrip_and_malformed_inputs() {
    let records = db_fixture().records;
    let bytes = arz::write(&records).unwrap();
    assert_eq!(arz::read(&bytes, |_| true).unwrap(), records);
    for end in [0, 1, 4, 12, 24, bytes.len() / 2, bytes.len() - 17] {
        assert!(arz::read(&bytes[..end], |_| true).is_err());
    }
    let mut corrupt = bytes.clone();
    corrupt[2] = 99;
    assert!(arz::read(&corrupt, |_| true).is_err());
    corrupt = bytes.clone();
    corrupt[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(arz::read(&corrupt, |_| true).is_err());
}
#[test]
fn transaction_backup_restore_and_stale_file() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("app");
    let target = root.path().join("player.gdc");
    fs::write(&target, b"original").unwrap();
    let original = grim_core::digest(b"original");
    let receipt =
        storage::commit(&data, &target, Some(&original), Some(b"edited"), "points").unwrap();
    assert!(receipt.completed);
    assert_eq!(fs::read(&target).unwrap(), b"edited");
    let (_, bytes) = storage::restore_payload(&data, &receipt.id).unwrap();
    assert_eq!(bytes.as_deref(), Some(b"original".as_slice()));
    assert!(storage::commit(
        &data,
        &target,
        Some(&original),
        Some(b"lost update"),
        "points"
    )
    .is_err());
    storage::commit(
        &data,
        &target,
        receipt.after.as_deref(),
        bytes.as_deref(),
        "points",
    )
    .unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"original");
    fs::write(
        data.join("backups").join(&receipt.id).join("before.bin"),
        b"damaged backup",
    )
    .unwrap();
    assert!(storage::restore_payload(&data, &receipt.id).is_err());
}
#[test]
fn new_mod_disable_is_recoverable() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("app");
    let target = root.path().join("mods/database.arz");
    let installed = storage::commit(&data, &target, None, Some(b"mod data"), "rules").unwrap();
    assert!(storage::is_owned(&data, &target, installed.after.as_deref().unwrap()).unwrap());
    let disabled =
        storage::commit(&data, &target, installed.after.as_deref(), None, "rules").unwrap();
    assert!(!target.exists());
    let (_, bytes) = storage::restore_payload(&data, &disabled.id).unwrap();
    storage::commit(&data, &target, None, bytes.as_deref(), "rules").unwrap();
    assert_eq!(fs::read(target).unwrap(), b"mod data");
}
#[test]
fn steam_paths_and_cloud_detection() {
    let vdf = r#""path" "D:\\SteamLibrary" "buildid" "1234""#;
    assert_eq!(
        discovery::vdf_value(vdf, "path").as_deref(),
        Some("D:\\SteamLibrary")
    );
    assert_eq!(
        discovery::vdf_value(vdf, "buildid").as_deref(),
        Some("1234")
    );
    assert!(discovery::cloud_path(&PathBuf::from(
        "C:\\Steam\\userdata\\1\\219990\\remote\\save\\main\\_x\\player.gdc"
    )));
    assert!(!discovery::cloud_path(&PathBuf::from(
        "/save/main/_x/player.gdc"
    )));
}
// Real game files are opt-in test inputs and are never included in the repo.
#[test]
fn real_save_edit_preserves_every_other_primitive() {
    let Ok(path) = std::env::var("GRIM_TEST_SAVE") else {
        return;
    };
    let bytes = fs::read(path).unwrap();
    let before = gdc::read(&bytes).unwrap();
    let target = gdc::Points {
        skill: before.points.skill + 7,
        attribute: before.points.attribute + 3,
        devotion: before.points.devotion + 1,
    };
    let (after, changed) = gdc::edit(&bytes, target, 10000).unwrap();
    assert_eq!(after.points, target);
    assert_eq!(after.name, before.name);
    assert_eq!(after.level, before.level);
    let (_, restored) = gdc::edit(&changed, before.points, 10000).unwrap();
    assert_eq!(restored, bytes);
    let mut damaged = bytes.clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 1;
    assert!(gdc::read(&damaged).is_err());
    assert!(gdc::read(&bytes[..bytes.len() - 8]).is_err());
    assert!(gdc::edit(
        &bytes,
        gdc::Points {
            skill: 10001,
            ..target
        },
        10000
    )
    .is_err());
    assert!(gdc::edit(&bytes, target, 0).is_err());
}
#[test]
fn real_game_profile_roundtrip() {
    let Ok(path) = std::env::var("GRIM_TEST_GAME") else {
        return;
    };
    let data = rules::load(PathBuf::from(path).as_path()).unwrap();
    let p = Profile {
        xp: 2.,
        drop: 2.,
        rarity: 2.,
        density: 2.,
        cooldown: 50.,
        ..Profile::default()
    };
    let built = rules::build(&data, &p).unwrap();
    assert!(built.summary.counts.values().all(|n| *n > 0));
    assert_eq!(built.summary.counts.len(), 5);
    let out = arz::read(&built.bytes, |_| true).unwrap();
    for (name, r) in out {
        let original = &data.records[&name];
        for (key, field) in &r.fields {
            if field != &original.fields[key] {
                assert!(
                    matches!(
                        key.as_str(),
                        "experienceEquation"
                            | "eliteExperienceEquation"
                            | "ultimateExperienceEquation"
                            | "challengeExperienceEquation"
                            | "spawnMin"
                            | "spawnMax"
                            | "skillCooldownTime"
                    ) || key.starts_with("limit")
                        || key.starts_with("chanceToEquipMisc"),
                    "Unexpected edit: {name}/{key}"
                );
            }
        }
    }
}

#[test]
fn service_previews_enforce_ownership_and_source_freshness() {
    use grim_core::service::Service;
    let root = tempfile::tempdir().unwrap();
    let game = root.path().join("game");
    fs::create_dir_all(game.join("database")).unwrap();
    let original = arz::write(&db_fixture().records).unwrap();
    let database = game.join("database/database.arz");
    fs::write(&database, &original).unwrap();
    let mut service = Service::new(root.path().join("app"));
    let profile = Profile {
        xp: 2.,
        ..Profile::default()
    };
    let first = service.preview_rules(&game, profile.clone()).unwrap();
    assert!(first.can_apply);
    let receipt = service.apply(&first.id).unwrap();
    assert!(receipt.completed);
    assert_eq!(fs::read(&database).unwrap(), original);
    let repeat = service.preview_rules(&game, profile.clone()).unwrap();
    assert!(!repeat.can_apply);
    let disabled = service.preview_disable(&game).unwrap();
    service.apply(&disabled.id).unwrap();
    assert!(!game.join("mods/database.arz").exists());
    let stale = service.preview_rules(&game, profile.clone()).unwrap();
    fs::write(&database, b"changed after preview").unwrap();
    assert!(service.apply(&stale.id).is_err());
    assert!(!game.join("mods/database.arz").exists());
    fs::write(&database, &original).unwrap();
    fs::write(game.join("mods/database.arz"), b"somebody else's mod").unwrap();
    let conflict = service.preview_rules(&game, profile).unwrap();
    assert!(!conflict.can_apply);
    let exported = service.export(&conflict.id, root.path()).unwrap();
    assert!(exported.join("mods/database.arz").is_file());
    assert!(service.apply(&conflict.id).is_err());
    assert_eq!(
        fs::read(game.join("mods/database.arz")).unwrap(),
        b"somebody else's mod"
    );
}
