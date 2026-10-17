use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub path: PathBuf,
    pub build_id: Option<String>,
    pub expansions: Vec<String>,
}
#[cfg(feature = "legacy-tools")]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEntry {
    pub path: PathBuf,
    pub cloud: bool,
    pub character: Option<crate::gdc::Character>,
    pub error: Option<String>,
}
#[cfg(feature = "legacy-tools")]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scan {
    pub platform: String,
    pub installations: Vec<Installation>,
    pub saves: Vec<SaveEntry>,
    pub game_running: bool,
    pub steam_running: bool,
}

pub fn archives(game: &Path) -> Result<Vec<PathBuf>> {
    ensure!(
        game.join("database/database.arz").is_file(),
        "Select the Grim Dawn folder containing database/database.arz"
    );
    let mut out = vec![game.join("database/database.arz")];
    for (folder, file) in [
        ("gdx1", "GDX1.arz"),
        ("gdx2", "GDX2.arz"),
        ("gdx3", "GDX3.arz"),
    ] {
        let path = game.join(folder).join("database").join(file);
        if path.is_file() {
            out.push(path);
        }
    }
    Ok(out)
}
// Steam's KeyValues files use quoted tokens; only path/buildid key-value
// pairs are needed. Braces and unquoted flags are intentionally ignored.
pub fn quoted_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut value = String::new();
    let mut inside = false;
    let mut escaped = false;
    for c in text.chars() {
        if inside {
            if escaped {
                value.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                out.push(std::mem::take(&mut value));
                inside = false;
            } else {
                value.push(c);
            }
        } else if c == '"' {
            inside = true;
        }
    }
    out
}
pub fn vdf_value(text: &str, key: &str) -> Option<String> {
    quoted_tokens(text)
        .windows(2)
        .find(|w| w[0] == key)
        .map(|w| w[1].clone())
}
fn children(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .map(|i| i.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}
fn steam_roots() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut roots = vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join("Library/Application Support/Steam"),
    ];
    for key in [
        "PROGRAMFILES(X86)",
        "PROGRAMFILES",
        "ProgramFiles(x86)",
        "ProgramFiles",
    ] {
        if let Some(p) = std::env::var_os(key) {
            roots.push(PathBuf::from(p).join("Steam"));
        }
    }
    // CrossOver bottles hold Windows Steam libraries.
    for bottle in children(&home.join("Library/Application Support/CrossOver/Bottles")) {
        roots.push(bottle.join("drive_c/Program Files (x86)/Steam"));
        roots.push(bottle.join("drive_c/Program Files/Steam"));
    }
    roots
        .into_iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
pub fn process_status() -> (bool, bool) {
    let (game, steam, _) = inspect_processes();
    (game, steam)
}
fn inspect_processes() -> (bool, bool, bool) {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always),
    );
    let game_names = ["grim dawn.exe", "grimdawn.exe"];
    let mut game = false;
    let mut steam = false;
    for p in sys.processes().values() {
        let name = p.name().to_string_lossy().to_lowercase();
        game |= game_names.contains(&name.as_str());
        steam |= matches!(name.as_str(), "steam" | "steam.exe");
        for arg in p.cmd().iter().take(5) {
            let normalized = arg.to_string_lossy().replace('\\', "/").to_lowercase();
            let base = normalized.rsplit('/').next().unwrap_or("");
            game |= game_names.contains(&base);
            steam |= matches!(base, "steam" | "steam.exe");
        }
    }
    (
        game,
        steam,
        sys.process(Pid::from_u32(std::process::id())).is_some(),
    )
}
pub fn require_closed(cloud: bool) -> Result<()> {
    let (game, steam, inspected) = inspect_processes();
    ensure!(
        inspected,
        "Unable to inspect running processes; file changes are disabled"
    );
    ensure!(
        !game,
        "Close Grim Dawn before changing its saves or gameplay rules"
    );
    ensure!(
        !cloud || !steam,
        "Close Steam before editing a cloud save so cloud sync cannot overwrite the change"
    );
    Ok(())
}
pub fn cloud_path(path: &Path) -> bool {
    let p = path.to_string_lossy().replace('\\', "/").to_lowercase();
    p.contains("/userdata/") && p.contains("/219990/remote/")
}
/// Require known vanilla command lines before retiring an unused legacy file.
pub fn require_no_basemods_session() -> Result<()> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    ensure!(
        sys.process(Pid::from_u32(std::process::id())).is_some(),
        "Unable to inspect game launch options"
    );
    for p in sys.processes().values() {
        let args: Vec<_> = p
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().replace('\\', "/").to_lowercase())
            .collect();
        let is_game = args
            .iter()
            .take(5)
            .any(|s| matches!(s.rsplit('/').next(), Some("grim dawn.exe" | "grimdawn.exe")))
            || matches!(
                p.name().to_string_lossy().to_lowercase().as_str(),
                "grim dawn.exe" | "grimdawn.exe"
            );
        if is_game {
            ensure!(
                !args.is_empty(),
                "Close the game once to remove its legacy profile; launch options are unavailable"
            );
            ensure!(!args.iter().any(|s|s.contains("basemods")),"This session loaded /basemods. Close the game, remove the old profile, then restart without /basemods for live training.");
        }
    }
    Ok(())
}
pub fn installation(path: &Path) -> Result<Installation> {
    let path = path.canonicalize().context("Game folder does not exist")?;
    let ars = archives(&path)?;
    let manifest = path
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("appmanifest_219990.acf"));
    let build_id = manifest
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| vdf_value(&s, "buildid"));
    Ok(Installation {
        path,
        build_id,
        expansions: ars
            .iter()
            .skip(1)
            .map(|p| {
                p.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect(),
    })
}
#[cfg(feature = "legacy-tools")]
pub fn scan(custom_game: Option<&Path>, custom_saves: Option<&Path>) -> Scan {
    let home = dirs::home_dir().unwrap_or_default();
    let roots = steam_roots();
    let mut libraries = BTreeSet::new();
    let mut save_roots = BTreeSet::new();
    let mut candidates = BTreeSet::new();
    for root in &roots {
        libraries.insert(root.clone());
        if let Ok(s) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for pair in quoted_tokens(&s).windows(2) {
                if pair[0] == "path" {
                    libraries.insert(PathBuf::from(&pair[1]));
                }
            }
        }
        for user in children(&root.join("userdata")) {
            save_roots.insert(user.join("219990/remote/save"));
        }
    }
    for library in libraries {
        candidates.insert(library.join("steamapps/common/Grim Dawn"));
        let users = library.join("steamapps/compatdata/219990/pfx/drive_c/users");
        for user in children(&users) {
            save_roots.insert(user.join("Documents/My Games/Grim Dawn/save"));
            save_roots.insert(user.join("My Documents/My Games/Grim Dawn/save"));
        }
    }
    for bottle in children(&home.join("Library/Application Support/CrossOver/Bottles")) {
        for user in children(&bottle.join("drive_c/users")) {
            save_roots.insert(user.join("Documents/My Games/Grim Dawn/save"));
        }
    }
    if let Some(doc) = dirs::document_dir() {
        save_roots.insert(doc.join("My Games/Grim Dawn/save"));
    }
    save_roots.insert(home.join("Documents/My Games/Grim Dawn/save"));
    if let Some(p) = custom_game {
        candidates.insert(p.into());
    }
    if let Some(p) = custom_saves {
        save_roots.insert(p.into());
    }
    let installations = candidates
        .iter()
        .filter_map(|p| installation(p).ok())
        .map(|i| (i.path.clone(), i))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect();
    let mut paths = BTreeSet::new();
    for root in save_roots {
        if root.is_file() {
            if let Ok(p) = root.canonicalize() {
                paths.insert(p);
            }
            continue;
        }
        for group in [root.clone(), root.join("main"), root.join("user")] {
            if group.join("player.gdc").is_file() {
                if let Ok(p) = group.join("player.gdc").canonicalize() {
                    paths.insert(p);
                }
            }
            for char_dir in children(&group) {
                if let Ok(p) = char_dir.join("player.gdc").canonicalize() {
                    paths.insert(p);
                }
            }
        }
    }
    let saves = paths
        .into_iter()
        .map(|path| {
            let result = fs::read(&path)
                .context("Unable to read save")
                .and_then(|b| crate::gdc::read(&b));
            let (character, error) = match result {
                Ok(c) => (Some(c), None),
                Err(e) => (None, Some(format!("{e:#}"))),
            };
            SaveEntry {
                cloud: cloud_path(&path),
                path,
                character,
                error,
            }
        })
        .collect();
    let (game_running, steam_running) = process_status();
    Scan {
        platform: std::env::consts::OS.into(),
        installations,
        saves,
        game_running,
        steam_running,
    }
}

/// Live mode only needs installation discovery; character saves are not parsed.
pub fn installations(custom: Option<&Path>) -> Vec<Installation> {
    let mut libraries = BTreeSet::new();
    for root in steam_roots() {
        if let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for pair in quoted_tokens(&text).windows(2) {
                if pair[0] == "path" {
                    libraries.insert(PathBuf::from(&pair[1]));
                }
            }
        }
        libraries.insert(root);
    }
    let mut candidates: BTreeSet<_> = libraries
        .into_iter()
        .map(|p| p.join("steamapps/common/Grim Dawn"))
        .collect();
    if let Some(p) = custom {
        candidates.insert(p.to_owned());
    }
    candidates
        .iter()
        .filter_map(|p| installation(p).ok())
        .map(|i| (i.path.clone(), i))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect()
}
