use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub id: String,
    pub target: PathBuf,
    pub kind: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub created: u64,
    pub completed: bool,
}
pub fn data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("GRIM_TRAINER_DATA_DIR") {
        return Ok(path.into());
    }
    Ok(dirs::data_local_dir()
        .context("No application data folder")?
        .join("GrimDawnTrainer"))
}
pub fn id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{now:x}-{:x}", std::process::id())
}
pub fn fingerprint(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(m) => {
            ensure!(
                m.is_file() && !m.file_type().is_symlink(),
                "The target must be a regular file, not a symbolic link"
            );
            Ok(Some(crate::digest(&fs::read(path)?)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn atomic(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing output directory")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    if let Ok(meta) = fs::metadata(path) {
        temp.as_file().set_permissions(meta.permissions())?;
    }
    temp.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic(path, &serde_json::to_vec_pretty(value)?)
}
pub fn receipts(data: &Path) -> Result<Vec<Receipt>> {
    let folder = data.join("backups");
    if !folder.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(folder)?.flatten() {
        if let Ok(bytes) = fs::read(entry.path().join("receipt.json")) {
            if let Ok(r) = serde_json::from_slice(&bytes) {
                out.push(r);
            }
        }
    }
    out.sort_by(|a: &Receipt, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
    Ok(out)
}
pub fn is_owned(data: &Path, target: &Path, hash: &str) -> Result<bool> {
    // Receipts store the canonical path written by commit_guarded.
    let canonical = target
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .zip(target.file_name())
        .map(|(parent, name)| parent.join(name));
    let target = canonical.as_deref().unwrap_or(target);
    Ok(receipts(data)?
        .iter()
        .any(|r| r.target == target && r.kind == "rules" && r.after.as_deref() == Some(hash)))
}
pub fn commit(
    data: &Path,
    target: &Path,
    expected: Option<&str>,
    output: Option<&[u8]>,
    kind: &str,
) -> Result<Receipt> {
    commit_guarded(data, target, expected, output, kind, || {
        crate::discovery::require_closed(crate::discovery::cloud_path(target))
    })
}
fn commit_guarded(
    data: &Path,
    target: &Path,
    expected: Option<&str>,
    output: Option<&[u8]>,
    kind: &str,
    guard: impl Fn() -> Result<()>,
) -> Result<Receipt> {
    guard()?;
    let parent = target.parent().context("Invalid target path")?;
    fs::create_dir_all(parent)?;
    let target = parent
        .canonicalize()?
        .join(target.file_name().context("Invalid filename")?);
    let lock_path = parent.join(format!(
        ".{}.grim-trainer.lock",
        target.file_name().unwrap().to_string_lossy()
    ));
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    fs4::FileExt::try_lock(&lock).context("Another trainer operation is using this file")?;
    ensure!(
        fingerprint(&target)?.as_deref() == expected,
        "The file changed after preview. Preview again before applying."
    );
    let old = if expected.is_some() {
        Some(fs::read(&target)?)
    } else {
        None
    };
    let after = output.map(crate::digest);
    ensure!(after.as_deref() != expected, "No file changes to apply");
    let receipt_dir = data.join("backups").join(id());
    fs::create_dir_all(&receipt_dir)?;
    let mut receipt = Receipt {
        id: receipt_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        target: target.clone(),
        kind: kind.into(),
        before: expected.map(str::to_owned),
        after,
        created: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        completed: false,
    };
    if let Some(old) = &old {
        atomic(&receipt_dir.join("before.bin"), old)?;
        ensure!(
            fingerprint(&receipt_dir.join("before.bin"))? == receipt.before,
            "Backup verification failed"
        );
    }
    if let Some(output) = output {
        atomic(&receipt_dir.join("after.bin"), output)?;
    }
    write_json(&receipt_dir.join("receipt.json"), &receipt)?;
    // Recheck after backup I/O, immediately before replacing the target.
    guard()?;
    ensure!(
        fingerprint(&target)?.as_deref() == expected,
        "The file changed while the backup was being made; nothing was applied"
    );
    if let Some(bytes) = output {
        atomic(&target, bytes)?;
    } else {
        fs::remove_file(&target)?;
    }
    ensure!(
        fingerprint(&target)? == receipt.after,
        "Written file failed verification. The original is preserved in {}",
        receipt_dir.display()
    );
    receipt.completed = true;
    write_json(&receipt_dir.join("receipt.json"), &receipt).context(
        "File applied, but recording completion failed; the backup receipt remains available",
    )?;
    Ok(receipt)
}
pub fn retire_unused_legacy(data: &Path, game: &Path) -> Result<Option<Receipt>> {
    let target = crate::discovery::installation(game)?
        .path
        .join("mods/database.arz");
    let Some(hash) = fingerprint(&target)? else {
        return Ok(None);
    };
    ensure!(is_owned(data,&target,&hash)?,"Another mod owns mods/database.arz. Live mode requires original game rules; disable that mod first.");
    commit_guarded(
        data,
        &target,
        Some(&hash),
        None,
        "rules",
        crate::discovery::require_no_basemods_session,
    )
    .map(Some)
}
pub fn restore_payload(data: &Path, id: &str) -> Result<(Receipt, Option<Vec<u8>>)> {
    ensure!(
        !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
        "Invalid backup identifier"
    );
    let folder = data.join("backups").join(id);
    let receipt: Receipt = serde_json::from_slice(&fs::read(folder.join("receipt.json"))?)?;
    ensure!(
        receipt.id == id,
        "Backup metadata does not match its folder"
    );
    let before = if let Some(hash) = &receipt.before {
        let bytes = fs::read(folder.join("before.bin"))?;
        ensure!(&crate::digest(&bytes) == hash, "Backup checksum failed");
        Some(bytes)
    } else {
        None
    };
    Ok((receipt, before))
}
pub fn export_new(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure!(!path.exists(), "Export destination already exists");
    fs::create_dir_all(path.parent().context("Invalid export path")?)?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    ensure!(
        fingerprint(path)?.as_deref() == Some(crate::digest(bytes).as_str()),
        "Export verification failed"
    );
    Ok(())
}
