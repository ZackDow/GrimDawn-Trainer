use crate::{discovery, storage};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    pub game: Option<PathBuf>,
    pub installations: Vec<discovery::Installation>,
    pub platform: &'static str,
}
fn settings(data: &Path) -> Result<Value> {
    match fs::read(data.join("settings.json")) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).context("Cannot read game settings")?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.into()),
    }
}
pub fn home(data: &Path) -> Result<Home> {
    let saved = settings(data)?;
    let selected = saved["game"].as_str().map(PathBuf::from);
    let installations = discovery::installations(selected.as_deref());
    let game = selected.or_else(|| installations.first().map(|i| i.path.clone()));
    Ok(Home {
        game,
        installations,
        platform: std::env::consts::OS,
    })
}
pub fn configure(data: &Path, game: Option<PathBuf>) -> Result<Home> {
    if let Some(path) = &game {
        discovery::installation(path)?;
    }
    let mut saved = settings(data)?;
    let object = saved
        .as_object_mut()
        .context("Invalid saved game settings")?;
    object.insert("game".into(), serde_json::to_value(game)?);
    storage::write_json(&data.join("settings.json"), &saved)?;
    home(data)
}
