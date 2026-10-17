use crate::{discovery, gdc, rules, storage};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub game: Option<PathBuf>,
    pub saves: Option<PathBuf>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    pub settings: Settings,
    pub scan: discovery::Scan,
    pub profiles: Vec<rules::Profile>,
    pub backups: Vec<storage::Receipt>,
    pub data_dir: PathBuf,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub target: PathBuf,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub can_apply: bool,
    pub rules: Option<rules::RuleSummary>,
    pub before_character: Option<gdc::Character>,
    pub after_character: Option<gdc::Character>,
    pub notes: Vec<String>,
}
struct Pending {
    view: Plan,
    output: Option<Vec<u8>>,
    sources: Vec<rules::Source>,
}
pub struct Service {
    data: PathBuf,
    pending: Option<Pending>,
}
impl Service {
    pub fn new(data: PathBuf) -> Self {
        Self {
            data,
            pending: None,
        }
    }
    pub fn home(&self) -> Result<Home> {
        let settings = match fs::read(self.data.join("settings.json")) {
            Ok(b) => serde_json::from_slice(&b).context("Unable to read saved settings")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => return Err(e.into()),
        };
        let scan = discovery::scan(settings.game.as_deref(), settings.saves.as_deref());
        Ok(Home {
            settings,
            scan,
            profiles: self.profiles()?,
            backups: storage::receipts(&self.data)?,
            data_dir: self.data.clone(),
        })
    }
    pub fn configure(&mut self, settings: Settings) -> Result<Home> {
        if let Some(game) = &settings.game {
            discovery::installation(game)?;
        }
        if let Some(saves) = &settings.saves {
            ensure!(saves.exists(), "Save location does not exist");
        }
        storage::write_json(&self.data.join("settings.json"), &settings)?;
        self.pending = None;
        self.home()
    }
    fn profiles(&self) -> Result<Vec<rules::Profile>> {
        match fs::read(self.data.join("profiles.json")) {
            Ok(b) => {
                let list: Vec<rules::Profile> =
                    serde_json::from_slice(&b).context("Unable to read saved profiles")?;
                for p in &list {
                    p.validate()?;
                }
                Ok(list)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save_profile(&self, profile: rules::Profile) -> Result<Vec<rules::Profile>> {
        profile.validate()?;
        let mut list = self.profiles()?;
        list.retain(|p| p.name != profile.name);
        list.push(profile);
        list.sort_by(|a, b| a.name.cmp(&b.name));
        storage::write_json(&self.data.join("profiles.json"), &list)?;
        Ok(list)
    }
    fn remember(
        &mut self,
        view: Plan,
        output: Option<Vec<u8>>,
        sources: Vec<rules::Source>,
    ) -> Plan {
        self.pending = Some(Pending {
            view: view.clone(),
            output,
            sources,
        });
        view
    }
    pub fn preview_rules(&mut self, game: &Path, profile: rules::Profile) -> Result<Plan> {
        self.pending = None;
        let game = discovery::installation(game)?.path;
        let data = rules::load(&game)?;
        let built = rules::build(&data, &profile)?;
        rules::verify_sources(&data.sources)?;
        let target = game.join("mods/database.arz");
        let before_hash = storage::fingerprint(&target)?;
        let owns_target = match before_hash.as_deref() {
            None => true,
            Some(hash) => storage::is_owned(&self.data, &target, hash)?,
        };
        let mut notes = built.summary.notes.clone();
        let already_installed = before_hash.as_deref() == Some(built.summary.sha256.as_str());
        let can_apply = owns_target && !already_installed;
        if already_installed {
            notes.push(
                "This exact gameplay profile is already installed. No file changes are needed."
                    .into(),
            );
        } else if !owns_target {
            notes.push("An existing base mod occupies mods/database.arz. Export this profile for manual merging; this app will not overwrite that mod.".into());
        }
        let view = Plan {
            id: storage::id(),
            kind: "rules".into(),
            title: format!("Install {}", profile.name),
            target,
            before_hash,
            after_hash: Some(built.summary.sha256.clone()),
            can_apply,
            rules: Some(built.summary),
            before_character: None,
            after_character: None,
            notes,
        };
        Ok(self.remember(view, Some(built.bytes), data.sources))
    }
    pub fn preview_points(
        &mut self,
        game: &Path,
        path: &Path,
        points: gdc::Points,
        expected: &str,
    ) -> Result<Plan> {
        self.pending = None;
        let path = path.canonicalize().context("Save file does not exist")?;
        let bytes = fs::read(&path)?;
        ensure!(
            crate::digest(&bytes) == expected,
            "Character changed since it was selected. Scan again before editing."
        );
        let before_character = gdc::read(&bytes)?;
        let game = discovery::installation(game)?.path;
        let sources = discovery::archives(&game)?
            .into_iter()
            .map(|path| {
                Ok(rules::Source {
                    sha256: crate::digest(&fs::read(&path)?),
                    path,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let cap = rules::devotion_cap(&game)?;
        let (after_character, output) = gdc::edit(&bytes, points, cap)?;
        ensure!(
            before_character.points != points,
            "No point changes selected"
        );
        let mut notes=vec![format!("Devotion spending limit remains {cap}; allocated points and constellation requirements are preserved."),"Only the unspent point balances and matching devotion total change. A verified backup is created before applying.".into()];
        if discovery::cloud_path(&path) {
            notes.push("This is a Steam cloud save. Close both Grim Dawn and Steam before applying or restoring it.".into());
        }
        let view = Plan {
            id: storage::id(),
            kind: "points".into(),
            title: format!("Update {}", before_character.name),
            target: path,
            before_hash: Some(before_character.sha256.clone()),
            after_hash: Some(after_character.sha256.clone()),
            can_apply: true,
            rules: None,
            before_character: Some(before_character),
            after_character: Some(after_character),
            notes,
        };
        Ok(self.remember(view, Some(output), sources))
    }
    pub fn preview_restore(&mut self, id: &str) -> Result<Plan> {
        self.pending = None;
        let (receipt, output) = storage::restore_payload(&self.data, id)?;
        let current = storage::fingerprint(&receipt.target)?;
        ensure!(
            current != receipt.before,
            "The file already matches this backup"
        );
        let mut notes=vec!["Restore the state from immediately before this operation. The current file will be backed up first.".into()];
        if current != receipt.after {
            notes.push("This file has changed since that operation; restoring will replace those newer changes. They will be preserved in a new backup.".into());
        }
        let view = Plan {
            id: storage::id(),
            kind: receipt.kind,
            title: "Restore backup".into(),
            target: receipt.target,
            before_hash: current,
            after_hash: receipt.before,
            can_apply: true,
            rules: None,
            before_character: None,
            after_character: None,
            notes,
        };
        Ok(self.remember(view, output, Vec::new()))
    }
    pub fn preview_disable(&mut self, game: &Path) -> Result<Plan> {
        self.pending = None;
        let target = discovery::installation(game)?
            .path
            .join("mods/database.arz");
        let current = storage::fingerprint(&target)?.context("No base-mod archive is installed")?;
        ensure!(
            storage::is_owned(&self.data, &target, &current)?,
            "The installed base mod was not created by this app"
        );
        let view=Plan{id:storage::id(),kind:"rules".into(),title:"Disable gameplay profile".into(),target,before_hash:Some(current),after_hash:None,can_apply:true,rules:None,before_character:None,after_character:None,notes:vec!["The trainer's active rules will be removed from the game's mod folder after a backup. Character points are unaffected.".into()]};
        Ok(self.remember(view, None, Vec::new()))
    }
    pub fn apply(&mut self, id: &str) -> Result<storage::Receipt> {
        let pending = self.pending.take().context("Preview the changes first")?;
        ensure!(
            pending.view.id == id,
            "This preview is stale; preview again"
        );
        ensure!(
            pending.view.can_apply,
            "This preview has no applicable changes or conflicts with another mod. Review its notes."
        );
        rules::verify_sources(&pending.sources)?;
        storage::commit(
            &self.data,
            &pending.view.target,
            pending.view.before_hash.as_deref(),
            pending.output.as_deref(),
            &pending.view.kind,
        )
    }
    pub fn export(&self, id: &str, folder: &Path) -> Result<PathBuf> {
        let pending = self
            .pending
            .as_ref()
            .context("Preview a gameplay profile first")?;
        ensure!(
            pending.view.id == id && pending.view.rules.is_some(),
            "Only a current gameplay profile can be exported"
        );
        rules::verify_sources(&pending.sources)?;
        let folder = folder
            .canonicalize()?
            .join(format!("GrimTrainer-{}", storage::id()));
        let bytes = pending.output.as_deref().context("No profile data")?;
        storage::export_new(&folder.join("mods/database.arz"), bytes)?;
        storage::export_new(
            &folder.join("profile-manifest.json"),
            &serde_json::to_vec_pretty(&pending.view.rules)?,
        )?;
        storage::export_new(&folder.join("README.txt"),b"Grim Dawn Trainer gameplay profile\n\nThis archive was generated from your own installed game databases.\nCopy mods/database.arz into the game's mods folder only if that file does not\nalready belong to another mod. Start Grim Dawn with /basemods.\nRebuild after game updates. Remove this archive to disable the rules.\nCharacter points are not included. Do not merge archives by concatenating them.\n")?;
        Ok(folder)
    }
}
