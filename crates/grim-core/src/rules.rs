use crate::{
    arz::{self, Record, Value},
    discovery,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub xp: f64,
    pub drop: f64,
    pub rarity: f64,
    pub density: f64,
    pub cooldown: f64,
    pub zero_cooldown: bool,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "My profile".into(),
            xp: 1.,
            drop: 1.,
            rarity: 1.,
            density: 1.,
            cooldown: 0.,
            zero_cooldown: false,
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.chars().count() <= 64,
            "Profile name must contain 1–64 characters"
        );
        for (name, value, max) in [
            ("XP", self.xp, 10.),
            ("Drop chance", self.drop, 10.),
            ("Rarity", self.rarity, 10.),
            ("Density", self.density, 5.),
        ] {
            ensure!(
                value.is_finite() && (1.0..=max).contains(&value),
                "{name} must be between 1 and {max}"
            );
        }
        ensure!(
            self.cooldown.is_finite() && (0.0..=90.).contains(&self.cooldown),
            "Cooldown reduction must be between 0 and 90 percent"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub category: String,
    pub record: String,
    pub field: String,
    pub before: Vec<Value>,
    pub after: Vec<Value>,
}
pub struct GameData {
    pub records: BTreeMap<String, Record>,
    pub qualities: BTreeMap<String, String>,
    pub sources: Vec<Source>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSummary {
    pub profile: Profile,
    pub sources: Vec<Source>,
    pub records: usize,
    pub counts: BTreeMap<String, usize>,
    pub changes: Vec<Change>,
    pub total_changes: usize,
    pub sha256: String,
    pub bytes: usize,
    pub notes: Vec<String>,
}
pub struct BuiltRules {
    pub summary: RuleSummary,
    pub bytes: Vec<u8>,
}

pub fn load(game: &Path) -> Result<GameData> {
    let mut records = BTreeMap::new();
    let mut qualities = BTreeMap::new();
    let mut sources = Vec::new();
    for path in discovery::archives(game)? {
        let bytes = fs::read(&path).with_context(|| format!("Read {}", path.display()))?;
        sources.push(Source {
            path,
            sha256: crate::digest(&bytes),
        });
        records.extend(arz::read(&bytes, |n| {
            n.starts_with("records/game/")
                || n.starts_with("records/proxies/")
                || n.starts_with("records/creatures/enemies/")
                || n.starts_with("records/skills/playerclass")
                || n.starts_with("records/items/loottables/")
                || n == "records/creatures/pc/playerlevels.dbr"
        })?);
        for (name, rec) in arz::read_selected(
            &bytes,
            |n| n.starts_with("records/items/"),
            |k| k == "itemClassification",
        )? {
            if let Some(quality) = rec.text("itemClassification") {
                qualities.insert(name, quality.into());
            }
        }
    }
    Ok(GameData {
        records,
        qualities,
        sources,
    })
}
pub use crate::game_data::devotion_cap;
fn suffix_number(key: &str, prefix: &str) -> bool {
    key.strip_prefix(prefix)
        .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
}
fn protected(name: &str) -> bool {
    [
        "boss",
        "hero",
        "quest",
        "nemesis",
        "bounty",
        "special",
        "event",
        "endless",
        "survival",
        "backup",
        "ambush",
        "test",
        "totem",
        "challenge",
    ]
    .iter()
    .any(|s| name.contains(s))
}
fn regular_pool(rec: &Record, db: &BTreeMap<String, Record>) -> bool {
    if !rec.name.contains("/pools/")
        || protected(&rec.name)
        || rec.text("templateName") != Some("database/templates/proxypool.tpl")
    {
        return false;
    }
    if !rec.number("spawnMax").is_some_and(|v| v > 1.) {
        return false;
    }
    let names: Vec<_> = rec
        .fields
        .iter()
        .filter(|(k, _)| suffix_number(k, "name"))
        .flat_map(|(_, f)| &f.values)
        .collect();
    !names.is_empty()
        && names.iter().all(|v| match v {
            Value::Text(n) => db
                .get(&n.replace('\\', "/").to_lowercase())
                .is_some_and(|r| {
                    r.class == "Monster"
                        && !protected(n)
                        && matches!(r.text("monsterClassification"), Some("Common" | "Champion"))
                }),
            _ => false,
        })
}
fn has_high_loot(
    name: &str,
    data: &GameData,
    seen: &mut BTreeSet<String>,
    cache: &mut BTreeMap<String, bool>,
) -> bool {
    let name = name.replace('\\', "/").to_lowercase();
    if let Some(v) = cache.get(&name) {
        return *v;
    }
    if let Some(q) = data.qualities.get(&name) {
        return matches!(q.as_str(), "Epic" | "Legendary");
    }
    if !seen.insert(name.clone()) || seen.len() > 64 {
        return false;
    }
    let result = data.records.get(&name).is_some_and(|r| {
        r.fields
            .iter()
            .filter(|(k, _)| k.as_str() == "records" || suffix_number(k, "lootName"))
            .flat_map(|(_, f)| &f.values)
            .any(|v| match v {
                Value::Text(s) => has_high_loot(s, data, seen, cache),
                _ => false,
            })
    });
    seen.remove(&name);
    cache.insert(name, result);
    result
}
fn update(
    rec: &mut Record,
    key: &str,
    category: &str,
    changes: &mut Vec<Change>,
    f: impl Fn(&Value) -> Result<Value>,
) -> Result<()> {
    if let Some(field) = rec.fields.get_mut(key) {
        let after = field.values.iter().map(f).collect::<Result<Vec<_>>>()?;
        if after != field.values {
            changes.push(Change {
                category: category.into(),
                record: rec.name.clone(),
                field: key.into(),
                before: field.values.clone(),
                after: after.clone(),
            });
            field.values = after;
        }
    }
    Ok(())
}
fn scale(
    rec: &mut Record,
    key: &str,
    category: &str,
    mult: f64,
    cap: Option<f64>,
    changes: &mut Vec<Change>,
) -> Result<()> {
    let kind = rec.fields.get(key).map(|f| f.kind).unwrap_or(0);
    update(rec, key, category, changes, |v| {
        let Value::Number(n) = v else {
            anyhow::bail!("Non-numeric field {key}")
        };
        ensure!(*n >= 0. && n.is_finite(), "Invalid base value in {key}");
        let mut value = n * mult;
        if let Some(cap) = cap {
            value = value.min(cap);
        }
        if kind == 0 {
            value = value.round();
        } else {
            value = (value as f32) as f64;
        }
        Ok(Value::Number(value))
    })
}
pub fn build(data: &GameData, profile: &Profile) -> Result<BuiltRules> {
    profile.validate()?;
    let mut changed = BTreeMap::new();
    let mut changes = Vec::new();
    let mut cache = BTreeMap::new();
    for (name, base) in &data.records {
        let mut rec = base.clone();
        let before = changes.len();
        if profile.xp != 1. && name == "records/game/experienceformulas.dbr" {
            for key in [
                "experienceEquation",
                "eliteExperienceEquation",
                "ultimateExperienceEquation",
                "challengeExperienceEquation",
            ] {
                ensure!(
                    rec.text(key).is_some(),
                    "XP formula {key} is missing in this game version"
                );
                update(&mut rec, key, "XP", &mut changes, |v| match v {
                    Value::Text(s) => Ok(Value::Text(format!("({s})*{}", profile.xp))),
                    _ => anyhow::bail!("Invalid XP formula"),
                })?;
            }
        }
        if profile.density != 1. && regular_pool(base, &data.records) {
            let keys: Vec<_> = rec
                .fields
                .keys()
                .filter(|k| {
                    matches!(k.as_str(), "spawnMin" | "spawnMax") || suffix_number(k, "limit")
                })
                .cloned()
                .collect();
            for key in keys {
                scale(
                    &mut rec,
                    &key,
                    "Density",
                    profile.density,
                    Some(250.),
                    &mut changes,
                )?;
            }
        }
        if base.class == "Monster" && (profile.drop != 1. || profile.rarity != 1.) {
            for slot in 1..=3 {
                let loot_prefix = format!("lootMisc{slot}Item");
                let weight_prefix = format!("chanceToEquipMisc{slot}Item");
                let choices: Vec<_> = base
                    .fields
                    .keys()
                    .filter_map(|key| {
                        if !suffix_number(key, &loot_prefix) {
                            return None;
                        }
                        let suffix = &key[loot_prefix.len()..];
                        let weight = format!("{weight_prefix}{suffix}");
                        let name = base.text(key)?;
                        if base.number(&weight).unwrap_or(0.) <= 0. {
                            return None;
                        }
                        Some((weight, name.to_string()))
                    })
                    .collect();
                // Do not multiply a slot which may award a direct quest item.
                if choices.is_empty()
                    || choices.iter().any(|(_, n)| {
                        !n.starts_with("records/items/loottables/") || n.contains("quest")
                    })
                {
                    continue;
                }
                if profile.drop != 1. {
                    scale(
                        &mut rec,
                        &format!("chanceToEquipMisc{slot}"),
                        "Loot chance",
                        profile.drop,
                        Some(100.),
                        &mut changes,
                    )?;
                }
                if profile.rarity != 1. {
                    let flags: Vec<_> = choices
                        .iter()
                        .map(|(_, n)| has_high_loot(n, data, &mut BTreeSet::new(), &mut cache))
                        .collect();
                    // Scaling every choice equally would have no effect.
                    if flags.iter().any(|v| *v) && flags.iter().any(|v| !*v) {
                        for ((key, _), high) in choices.iter().zip(flags) {
                            if high {
                                scale(&mut rec, key, "Rarity", profile.rarity, None, &mut changes)?;
                            }
                        }
                    }
                }
            }
        }
        if (profile.cooldown != 0. || profile.zero_cooldown)
            && name.starts_with("records/skills/playerclass")
            && !name.contains("/pets/")
            && base.class.starts_with("Skill_")
            && !base.class.contains("Passive")
            && !base.class.contains("OnHit")
            && !base.class.contains("OnAttack")
            && !base.class.contains("OnCrit")
            && !base.class.contains("Transmuter")
            && base.fields.get("skillCooldownTime").is_some_and(|f| {
                f.values
                    .iter()
                    .all(|v| matches!(v,Value::Number(n) if *n>=0.))
            })
        {
            scale(
                &mut rec,
                "skillCooldownTime",
                "Cooldown",
                if profile.zero_cooldown {
                    0.
                } else {
                    1. - profile.cooldown / 100.
                },
                None,
                &mut changes,
            )?;
        }
        if changes.len() > before {
            changed.insert(name.clone(), rec);
        }
    }
    let mut counts = BTreeMap::new();
    for change in &changes {
        *counts.entry(change.category.clone()).or_insert(0) += 1;
    }
    for (category, on) in [
        ("XP", profile.xp != 1.),
        ("Loot chance", profile.drop != 1.),
        ("Rarity", profile.rarity != 1.),
        ("Density", profile.density != 1.),
        ("Cooldown", profile.cooldown != 0. || profile.zero_cooldown),
    ] {
        ensure!(
            !on || counts.contains_key(category),
            "No supported {category} records matched this installation. The profile was not built."
        );
    }
    let bytes = arz::write(&changed)?;
    // Verify every unedited field, not only the selected values.
    let decoded = arz::read(&bytes, |_| true)?;
    ensure!(decoded == changed, "Archive verification failed");
    let mut notes = vec![
        "Main Campaign: start the game with /basemods after installing.".into(),
        "Changes apply to newly loaded sessions and newly spawned monsters.".into(),
    ];
    if profile.drop != 1. {
        notes.push("Drop chance affects eligible miscellaneous monster loot slots, capped at 100%; equipped combat gear and direct quest-item slots are preserved.".into());
    }
    if profile.rarity != 1. {
        notes.push("Rarity biases existing tables containing epic/legendary items. Level restrictions and fallback loot still apply; it is not a guaranteed item drop.".into());
    }
    if profile.density != 1. {
        notes.push("Density changes eligible ordinary pools and their ordinary entry limits; champion counts, boss/quest/event pools and single-spawn pools are preserved. Counts are capped at 250.".into());
    }
    if profile.cooldown != 0. || profile.zero_cooldown {
        notes.push("Cooldown changes player mastery active-skill base times. Item/devotion procs, energy costs, animations and gear/transmuter modifiers retain their rules.".into());
    }
    Ok(BuiltRules {
        summary: RuleSummary {
            profile: profile.clone(),
            sources: data.sources.clone(),
            records: changed.len(),
            counts,
            total_changes: changes.len(),
            changes: changes.into_iter().take(150).collect(),
            sha256: crate::digest(&bytes),
            bytes: bytes.len(),
            notes,
        },
        bytes,
    })
}
pub fn verify_sources(sources: &[Source]) -> Result<()> {
    for source in sources {
        ensure!(
            crate::digest(&fs::read(&source.path)?) == source.sha256,
            "Game database changed after preview. Scan and preview again."
        );
    }
    Ok(())
}
