//! Session transport. The injected Windows runtime owns the independent timeout.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Runner {
    pub wine: Option<PathBuf>,
    pub prefix: Option<PathBuf>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Modifiers {
    pub xp: f32,
    pub drop: f32,
    pub density: f32,
    pub cooldown: f32,
    pub zero_cooldown: bool,
    pub enabled: bool,
    pub prevent_broken: bool,
}
impl Default for Modifiers {
    fn default() -> Self {
        Self {
            xp: 1.,
            drop: 1.,
            density: 1.,
            cooldown: 0.,
            zero_cooldown: false,
            enabled: false,
            prevent_broken: true,
        }
    }
}
impl Modifiers {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1. ..=10.).contains(&self.xp)
                && (1. ..=10.).contains(&self.drop)
                && (1. ..=5.).contains(&self.density)
                && (0. ..=90.).contains(&self.cooldown),
            "Invalid live multiplier range"
        );
        Ok(())
    }
    pub fn line(&self) -> Result<String> {
        self.validate()?;
        Ok(format!(
            "set {} {} {} {} {} {} {}\n",
            self.xp,
            self.drop,
            self.density,
            self.cooldown,
            u8::from(self.zero_cooldown),
            u8::from(self.enabled),
            u8::from(self.prevent_broken)
        ))
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortalTarget {
    pub player: String,
    pub player_id: String,
    pub character_name: String,
    pub session_id: String,
    pub difficulty: u32,
}
impl PortalTarget {
    fn validate(&self, status: &Value, session_id: &str) -> Result<()> {
        ensure!(
            self.session_id == session_id,
            "The connection changed; refresh and try again"
        );
        ensure!(
            status["state"] == "ready"
                && self.player != "0"
                && status["playerCookie"].as_str() == Some(&self.player),
            "Load a character before unlocking portals"
        );
        // New characters legitimately use an all-zero save ID. Their live
        // connection, player epoch and displayed name still bind the action.
        ensure!(
            self.player_id.len() == 32
                && self.player_id.bytes().all(|b| b.is_ascii_hexdigit())
                && status["playerId"].as_str() == Some(&self.player_id)
                && !self.character_name.is_empty()
                && status["characterName"].as_str() == Some(&self.character_name),
            "The loaded character changed; refresh and try again"
        );
        ensure!(
            self.difficulty <= 2 && status["difficulty"].as_u64() == Some(self.difficulty as u64),
            "The game difficulty changed; refresh and try again"
        );
        ensure!(
            status["portalTotal"].as_u64().unwrap_or(0) > 0,
            "No campaign riftgates found; enter the playable world"
        );
        Ok(())
    }
}
struct Link {
    input: Mutex<Option<ChildStdin>>,
    status: Mutex<Value>,
    stop: AtomicBool,
}
pub struct Client {
    child: Child,
    link: Arc<Link>,
    pump: Option<thread::JoinHandle<()>>,
    reader: Option<thread::JoinHandle<()>>,
    next_command: i32,
    game: PathBuf,
    current: Modifiers,
    session_id: String,
}
fn write(link: &Link, line: &str) -> Result<()> {
    let mut input = link
        .input
        .lock()
        .map_err(|_| anyhow::anyhow!("Runtime pipe unavailable"))?;
    let input = input.as_mut().context("Runtime has disconnected")?;
    input.write_all(line.as_bytes())?;
    input.flush()?;
    Ok(())
}
fn windows_path(path: &Path) -> String {
    if cfg!(target_os = "windows") {
        path.to_string_lossy().into_owned()
    } else {
        format!("Z:{}", path.to_string_lossy().replace('/', "\\"))
    }
}
#[cfg(target_os = "linux")]
fn running_wine(game: &Path) -> Option<(PathBuf, PathBuf)> {
    let module = game.join("x64/Game.dll").to_string_lossy().into_owned();
    for process in fs::read_dir("/proc").ok()?.flatten() {
        if !process
            .file_name()
            .to_string_lossy()
            .chars()
            .all(|c| c.is_ascii_digit())
        {
            continue;
        }
        let path = process.path();
        let Ok(maps) = fs::read_to_string(path.join("maps")) else {
            continue;
        };
        if !maps.contains(&module) {
            continue;
        }
        let env = fs::read(path.join("environ")).ok()?;
        let prefix = env
            .split(|b| *b == 0)
            .find_map(|s| s.strip_prefix(b"WINEPREFIX="))
            .map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned()))?;
        let exe = fs::read_link(path.join("exe")).ok()?;
        let wine = exe
            .ancestors()
            .map(|p| p.join("bin/wine"))
            .find(|p| p.is_file())?;
        return Some((wine, prefix));
    }
    None
}
fn launch(game: &Path, resources: &Path, runner: &Runner) -> Result<Command> {
    #[cfg(target_os = "windows")]
    let _ = (game, runner);
    let bridge = resources.join("grim-bridge.exe");
    let dll = resources.join("grim-session-v8.dll");
    ensure!(bridge.is_file()&&dll.is_file(),"Runtime files are missing. Keep the runtime folder with the Linux executable, or reinstall the package.");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new(&bridge);
        c.creation_flags(0x08000000);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        #[cfg(target_os = "linux")]
        let detected = running_wine(game);
        #[cfg(not(target_os = "linux"))]
        let detected: Option<(PathBuf, PathBuf)> = None;
        let wine=runner.wine.clone().or_else(||detected.as_ref().map(|d|d.0.clone())).or_else(||{
            let p=PathBuf::from("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine");p.is_file().then_some(p)
        }).context("Could not find the running game's Wine/CrossOver command. Open the game, or select the runner and prefix in Advanced connection.")?;
        ensure!(wine.is_file(), "Selected Wine command does not exist");
        let prefix = runner
            .prefix
            .clone()
            .or_else(|| detected.as_ref().map(|d| d.1.clone()))
            .or_else(|| {
                game.ancestors()
                    .find(|p| {
                        p.parent()
                            .and_then(Path::file_name)
                            .is_some_and(|n| n == "Bottles")
                    })
                    .map(Path::to_path_buf)
            })
            .context("Select the same Wine/Proton prefix or CrossOver bottle used by the game")?;
        ensure!(prefix.is_dir(), "Selected Wine prefix does not exist");
        let mut c = Command::new(wine);
        c.env("WINEPREFIX", &prefix).env("WINEDEBUG", "-all");
        #[cfg(target_os = "macos")]
        {
            c.arg("--bottle")
                .arg(prefix.file_name().context("Invalid bottle path")?);
        }
        c.arg(&bridge);
        c
    };
    cmd.arg("--attach").arg(windows_path(&dll));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    Ok(cmd)
}
impl Client {
    pub fn connect(game: &Path, resources: &Path, runner: &Runner) -> Result<Self> {
        let game = crate::discovery::installation(game)?.path;
        crate::discovery::require_no_basemods_session()?;
        ensure!(
            crate::digest(&fs::read(game.join("x64/Game.dll"))?)
                == "07775a297050e84a846af1182731700614fb8b7bb41cca46b37fd24c90529387",
            "Live runtime does not support this Game.dll build yet"
        );
        let resources = stage_resources(resources, &crate::storage::data_dir()?)?;
        let mut child = launch(&game, &resources, runner)?
            .spawn()
            .context("Could not start the Windows runtime bridge")?;
        let output = child.stdout.take().context("No bridge output")?;
        let input = child.stdin.take().context("No bridge input")?;
        let link = Arc::new(Link {
            input: Mutex::new(Some(input)),
            status: Mutex::new(json!({"state":"connecting","error":""})),
            stop: AtomicBool::new(false),
        });
        let read_link = link.clone();
        let reader = thread::spawn(move || {
            for line in BufReader::new(output)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if line.len() > 16_384 {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if value.is_object() {
                        if let Ok(mut status) = read_link.status.lock() {
                            if let (Some(a), Some(b)) = (status.as_object_mut(), value.as_object())
                            {
                                for (k, v) in b {
                                    a.insert(k.clone(), v.clone());
                                }
                            } else {
                                *status = value;
                            }
                        }
                    }
                }
            }
            if let Ok(mut status) = read_link.status.lock() {
                if status["state"] != "error" && status["state"] != "disconnected" {
                    status["state"] = json!("disconnected");
                }
            }
            read_link.stop.store(true, Ordering::Release);
        });
        let beat_link = link.clone();
        let pump = thread::spawn(move || {
            while !beat_link.stop.load(Ordering::Acquire) {
                // Native heartbeat keeps working while the game is foreground
                // and WebKit throttles background JavaScript timers.
                if write(&beat_link, "ping\n").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(250));
            }
        });
        Ok(Self {
            child,
            link,
            pump: Some(pump),
            reader: Some(reader),
            next_command: 1,
            game,
            current: Modifiers::default(),
            session_id: crate::storage::id(),
        })
    }
    pub fn poll(&mut self) -> Result<Value> {
        let mut status = self
            .link
            .status
            .lock()
            .map_err(|_| anyhow::anyhow!("Runtime status unavailable"))?
            .clone();
        if self.child.try_wait()?.is_some() && status["state"] != "error" {
            status["state"] = json!("disconnected");
        }
        if status["state"] == "error" {
            self.disconnect();
        }
        if status["state"] == "disconnected" || status["state"] == "error" {
            self.current = Modifiers::default();
        }
        status["modifiers"] = serde_json::to_value(self.current)?;
        status["sessionId"] = json!(self.session_id);
        status["legacyProfileInstalled"] = json!(self.game.join("mods/database.arz").exists());
        Ok(status)
    }
    pub fn set(&mut self, values: Modifiers) -> Result<()> {
        let result = self.set_inner(values);
        if result.is_err() {
            self.disconnect();
        }
        result
    }
    fn set_inner(&mut self, values: Modifiers) -> Result<()> {
        let status = self.poll()?;
        ensure!(
            status["state"] == "ready",
            "Connect to the game before applying live modifiers"
        );
        // Do not silently stack a persistent override on the new session engine.
        ensure!(
            !values.enabled || !self.game.join("mods/database.arz").exists(),
            "Remove the old file profile before using live modifiers."
        );
        write(&self.link, &values.line()?)?;
        self.current = values;
        Ok(())
    }
    pub fn grant(&mut self, player: &str, points: crate::model::Points) -> Result<i32> {
        let status = self.poll()?;
        ensure!(
            status["state"] == "ready"
                && status["playerCookie"].as_str() == Some(player)
                && player != "0",
            "Load a character and refresh before granting points"
        );
        ensure!(
            points.skill <= 1000 && points.attribute <= 1000 && points.devotion <= 1000,
            "A single grant is limited to 1000 points per pool"
        );
        let cap = crate::game_data::devotion_cap(&self.game)?;
        let ack = status["acknowledged"].as_i64().unwrap_or(0) as i32;
        self.next_command = self.next_command.max(ack + 1);
        let command = self.next_command;
        self.next_command += 1;
        let cookie = player.parse::<u64>().context("Invalid character session")?;
        write(
            &self.link,
            &format!(
                "grant {command} {cookie} {} {} {} {cap}\n",
                points.skill, points.attribute, points.devotion
            ),
        )?;
        Ok(command)
    }
    pub fn unlock_portals(&mut self, target: &PortalTarget) -> Result<i32> {
        let status = self.poll()?;
        target.validate(&status, &self.session_id)?;
        let ack = status["acknowledged"].as_i64().unwrap_or(0) as i32;
        self.next_command = self.next_command.max(ack + 1);
        let command = self.next_command;
        self.next_command += 1;
        let cookie = target
            .player
            .parse::<u64>()
            .context("Invalid character session")?;
        write(
            &self.link,
            &format!(
                "portals {command} {cookie} {} {}\n",
                target.difficulty, target.player_id
            ),
        )?;
        Ok(command)
    }
    pub fn disconnect(&mut self) -> Value {
        let _ = write(&self.link, "stop\n");
        self.link.stop.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Err(_) => break,
                _ => (),
            };
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                break;
            }
            thread::sleep(Duration::from_millis(40));
        }
        let _ = self.child.wait();
        let _ = self.link.input.lock().map(|mut i| i.take());
        if let Some(t) = self.pump.take() {
            let _ = t.join();
        }
        if let Some(t) = self.reader.take() {
            let _ = t.join();
        }
        self.link
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| json!({"state":"disconnected"}))
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.disconnect();
    }
}

pub fn resources(candidates: &[PathBuf]) -> Result<PathBuf> {
    for p in candidates {
        if p.join("grim-bridge.exe").is_file() && p.join("grim-session-v8.dll").is_file() {
            return Ok(p.clone());
        }
    }
    bail!("Runtime resource folder was not found")
}
/// Steam's container can hide host /usr/lib. Stage immutable helper versions
/// under the user's data directory, which is shared with the game container.
fn stage_resources(source: &Path, data: &Path) -> Result<PathBuf> {
    let names = ["grim-bridge.exe", "grim-session-v8.dll"];
    let files = names
        .iter()
        .map(|n| fs::read(source.join(n)).map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?;
    let key = crate::digest(&files.concat());
    let output = data.join("runtime").join(&key[..20]);
    fs::create_dir_all(&output)?;
    for (name, bytes) in names.iter().zip(files) {
        let target = output.join(name);
        if target.exists() {
            ensure!(
                crate::digest(&fs::read(&target)?) == crate::digest(&bytes),
                "Staged runtime integrity check failed"
            );
        } else {
            crate::storage::export_new(&target, &bytes)?;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_rejects_invalid_settings() {
        assert!(Modifiers {
            xp: f32::NAN,
            ..Modifiers::default()
        }
        .validate()
        .is_err());
        assert!(Modifiers {
            density: 6.,
            ..Modifiers::default()
        }
        .validate()
        .is_err());
        assert_eq!(Modifiers::default().line().unwrap(), "set 1 1 1 0 0 0 1\n");
    }
    #[test]
    fn portal_scope_accepts_new_character_but_rejects_stale_identity() {
        let target = PortalTarget {
            player: "1".into(),
            player_id: "0".repeat(32),
            character_name: "Nexus".into(),
            session_id: "session-a".into(),
            difficulty: 0,
        };
        let status = json!({"state":"ready","playerCookie":"1","playerId":"0".repeat(32),"characterName":"Nexus","difficulty":0,"portalTotal":72});
        assert!(target.validate(&status, "session-a").is_ok());
        assert!(target.validate(&status, "session-b").is_err());
        for (key, value) in [
            ("characterName", json!("Other")),
            ("playerCookie", json!("3")),
            ("difficulty", json!(1)),
        ] {
            let mut changed = status.clone();
            changed[key] = value;
            assert!(target.validate(&changed, "session-a").is_err());
        }
    }
}
