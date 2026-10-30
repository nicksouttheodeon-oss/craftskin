//! Sidecar install adapters. Never edit installed executables or the launcher.
use crate::presets::{Palette, Style, validate_tree};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub type Result<T> = std::result::Result<T, String>;
const MAX_FILE: u64 = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct BackupInfo {
    file: String,
    original_existed: bool,
    applied_sha256: String,
}

fn checksum(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn token() -> String { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos().to_string() }
fn read_small(path: &Path) -> Result<Option<Vec<u8>>> {
    if path.is_symlink() { return Err("Refusing a symlinked file".into()); }
    if !path.exists() { return Ok(None); }
    let meta = path.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_FILE { return Err(format!("Unexpected or oversized file: {}", path.display())); }
    fs::read(path).map(Some).map_err(|e| e.to_string())
}
fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("File has no parent folder")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(".craftskin-{}.tmp", token()));
    let write_result = (|| -> Result<()> {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e| e.to_string())?;
        f.write_all(data).and_then(|_| f.sync_all()).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if write_result.is_err() { let _ = fs::remove_file(tmp); }
    write_result
}
fn install_with_backup(path: &Path, new_data: Vec<u8>) -> Result<PathBuf> {
    let original = read_small(path)?;
    if original.as_deref() == Some(new_data.as_slice()) { return Err("File already has these changes".into()); }
    let parent = path.parent().ok_or("File has no parent folder")?;
    let folder = parent.join(".craftskin-backups");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let stem = path.file_name().ok_or("Invalid filename")?.to_string_lossy();
    let prefix = format!("{}-{}", stem, token());
    let backup = folder.join(format!("{prefix}.bak"));
    let metadata = folder.join(format!("{prefix}.json"));
    if let Some(ref old) = original { fs::write(&backup, old).map_err(|e| e.to_string())?; }
    let info = BackupInfo { file: path.to_string_lossy().to_string(), original_existed: original.is_some(), applied_sha256: checksum(&new_data) };
    fs::write(&metadata, serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    write_atomic(path, &new_data)?;
    Ok(metadata)
}
pub fn restore_latest(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().ok_or("Invalid file location")?;
    let folder = parent.join(".craftskin-backups");
    let name = path.file_name().ok_or("Invalid filename")?.to_string_lossy();
    let mut files: Vec<_> = fs::read_dir(&folder).map_err(|_| "No backups exist for this file")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{name}-"))) && p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let info_file = files.pop().ok_or("No matching CraftSkin backup")?;
    let info: BackupInfo = serde_json::from_slice(&fs::read(&info_file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if info.file != path.to_string_lossy() { return Err("Backup belongs to a different path".into()); }
    let current = read_small(path)?.ok_or("Installed file has disappeared")?;
    if checksum(&current) != info.applied_sha256 { return Err("The file changed since installation; refusing to overwrite your edits".into()); }
    if info.original_existed {
        let backup = info_file.with_extension("bak");
        let old = fs::read(&backup).map_err(|e| format!("Cannot read saved backup: {e}"))?;
        write_atomic(path, &old)?;
    } else { fs::remove_file(path).map_err(|e| e.to_string())?; }
    // Remove metadata only after successfully restoring; otherwise older metadata might be replayed.
    fs::remove_file(&info_file).map_err(|e| e.to_string())?;
    Ok(info_file)
}

pub fn install_workspace(path: &Path, style: Style) -> Result<PathBuf> {
    let tree = style.tree();
    validate_tree(&tree)?;
    let mut obj: Value = match read_small(path)? {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("Invalid FilmCraft workspace JSON: {e}"))?,
        None => json!({ "saved": [], "current": "" }),
    };
    let root = obj.as_object_mut().ok_or("Workspace JSON must contain an object")?;
    let saved = root.entry("saved").or_insert_with(|| json!([])).as_array_mut().ok_or("Workspace saved field must be an array")?;
    let name = style.workspace_name();
    saved.retain(|v| v.get("name").and_then(Value::as_str) != Some(name.as_str()));
    // `workspace` intentionally absent: FilmCraft's saved_workspace() reads `layout`
    // through legacy::layout when the new-style workspace field is null.
    saved.push(json!({ "name": name, "layout": tree, "floating": null }));
    root.insert("current".into(), json!(name));
    let bytes = serde_json::to_vec_pretty(&obj).map_err(|e| e.to_string())?;
    install_with_backup(path, bytes)
}

pub fn install_accent(path: &Path, palette: Palette) -> Result<PathBuf> {
    let mut obj: Value = match read_small(path)? {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("Invalid preferences: {e}"))?,
        None => json!({ "version": 2 }),
    };
    let root = obj.as_object_mut().ok_or("Preferences root must be an object")?;
    if root.get("version").and_then(Value::as_i64) != Some(2) { return Err("Unknown FilmCraft preferences schema; no changes made".into()); }
    let appearance = root.entry("appearance").or_insert_with(|| json!({})).as_object_mut().ok_or("Appearance preferences are invalid")?;
    appearance.insert("appearanceMode".into(), json!("dark"));
    appearance.insert("darkTheme".into(), json!("darkest"));
    appearance.insert("colorTheme".into(), json!("darkest"));
    appearance.insert("lightTheme".into(), json!("light"));
    appearance.insert("highlightColor".into(), json!(palette.accent));
    install_with_backup(path, serde_json::to_vec_pretty(&obj).map_err(|e| e.to_string())?)
}

pub fn default_filmcraft_data() -> PathBuf {
    if let Some(folder) = std::env::var_os("FILMCRAFT_DATA_DIR") { return PathBuf::from(folder); }
    if cfg!(target_os = "windows") { return std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("FilmCraft"); }
    if cfg!(target_os = "macos") { return home().join("Library/Application Support/FilmCraft"); }
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share")).join("filmcraft")
}
fn home() -> PathBuf { std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")) }

/// Transaction record for a *single* guarded FilmCraft source checkout.
#[derive(Serialize, Deserialize)]
struct BatchEntry {
    relative: String,
    existed: bool,
    original_sha256: Option<String>,
    installed_sha256: String,
    backup_name: String,
}
#[derive(Serialize, Deserialize)]
struct BatchManifest { schema: u32, entries: Vec<BatchEntry> }

/// Stage every backup before editing a file. Roll back on any failed source write.
pub fn install_batch(root: &Path, changes: &[(PathBuf, Vec<u8>)]) -> Result<PathBuf> {
    if changes.is_empty() { return Err("No changes planned".into()); }
    let mut prepared = Vec::new();
    for (path, bytes) in changes {
        if !path.starts_with(root) { return Err("Refusing an edit outside the source checkout".into()); }
        let original = read_small(path)?;
        if original.as_deref() == Some(bytes) { return Err(format!("Already patched: {}", path.display())); }
        prepared.push((path.clone(), original, bytes.clone()));
    }
    let dir = root.join(".craftskin-backups").join(format!("integration-{}", token()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for (i, (path, old, new_data)) in prepared.iter().enumerate() {
        let name = format!("{i:02}.orig");
        if let Some(bytes) = old { fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?; }
        entries.push(BatchEntry {
            relative: path.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().into_owned(),
            existed: old.is_some(), original_sha256: old.as_deref().map(checksum),
            installed_sha256: checksum(new_data), backup_name: name,
        });
    }
    let manifest = BatchManifest { schema: 1, entries };
    let metadata = dir.join("manifest.json");
    write_atomic(&metadata, &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)?;
    let mut edited = Vec::new();
    for (path, _, new_data) in &prepared {
        match write_atomic(path, new_data) {
            Ok(()) => edited.push(path.clone()),
            Err(e) => {
                for edited_path in edited.into_iter().rev() {
                    if let Some((_, original, _)) = prepared.iter().find(|(p, _, _)| *p == edited_path) {
                        if let Some(old) = original { let _ = write_atomic(&edited_path, old); }
                        else { let _ = fs::remove_file(&edited_path); }
                    }
                }
                return Err(format!("Write failed; attempted rollback: {e}. Backups: {}", dir.display()));
            }
        }
    }
    Ok(metadata)
}

/// Restore exactly the last integrated source snapshot only if no external edits intervened.
pub fn restore_batch(root: &Path) -> Result<PathBuf> {
    let backroot = root.join(".craftskin-backups");
    let mut dirs: Vec<_> = fs::read_dir(&backroot).map_err(|_| "No integrated source backups found")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("integration-")))
        .collect();
    dirs.sort();
    let dir = dirs.pop().ok_or("No integrated source backups found")?;
    let metadata = dir.join("manifest.json");
    let manifest: BatchManifest = serde_json::from_slice(&fs::read(&metadata).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if manifest.schema != 1 || manifest.entries.is_empty() || manifest.entries.len() > 32 { return Err("Unexpected backup manifest; refusing restore".into()); }
    let mut old_files = Vec::new();
    // Preflight every file and backup before touching any files.
    for e in &manifest.entries {
        let path = root.join(&e.relative);
        if !path.starts_with(root) || Path::new(&e.relative).components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err("Unsafe path in manifest".into());
        }
        let current = read_small(&path)?.ok_or(format!("Modified source file missing: {}", path.display()))?;
        if checksum(&current) != e.installed_sha256 { return Err(format!("Source changed since install: {}. Restore manually or use git", path.display())); }
        let original = if e.existed {
            let bytes = fs::read(dir.join(&e.backup_name)).map_err(|e| e.to_string())?;
            if Some(checksum(&bytes)) != e.original_sha256 { return Err("Backup checksum mismatch".into()); }
            Some(bytes)
        } else { None };
        old_files.push((path, original, current));
    }
    let mut restored: Vec<PathBuf> = Vec::new();
    for (path, original, current) in &old_files {
        let res = match original { Some(o) => write_atomic(path, o), None => fs::remove_file(path).map_err(|e| e.to_string()) };
        if let Err(e) = res {
            for edited in restored.into_iter().rev() {
                if let Some((_, _, before)) = old_files.iter().find(|(p, _, _)| *p == edited) { let _ = write_atomic(&edited, before); }
            }
            return Err(format!("Restoration failed; attempted rollback: {e}"));
        }
        restored.push(path.clone());
    }
    fs::remove_file(&metadata).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn default_skin_config() -> PathBuf {
    if let Some(folder) = std::env::var_os("CRAFTSKIN_CONFIG") { return PathBuf::from(folder); }
    if cfg!(target_os = "windows") { return std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("CraftSkin/filmcraft.json"); }
    if cfg!(target_os = "macos") { return home().join("Library/Application Support/CraftSkin/filmcraft.json"); }
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config")).join("craftskin/filmcraft.json")
}

pub fn install_skin_config(path: &Path, style: Style) -> Result<PathBuf> {
    let value = crate::skinpack::preset(style);
    crate::skinpack::validate(&value)?;
    install_with_backup(path, serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    #[test]
    fn round_trip_multiple_files_and_detect_external_edits() {
        let p = std::env::temp_dir().join(format!("craftskin-test-{}", token()));
        fs::create_dir_all(&p).unwrap();
        let a = p.join("a.rs");
        let b = p.join("b.rs");
        fs::write(&a, b"before").unwrap();
        let changes = vec![(a.clone(), b"after".to_vec()), (b.clone(), b"added".to_vec()),
            (p.join("c"), b"x".to_vec()), (p.join("d"), b"x".to_vec()), (p.join("e"), b"x".to_vec()),
            (p.join("f"), b"x".to_vec()), (p.join("g"), b"x".to_vec())];
        install_batch(&p, &changes).unwrap();
        assert_eq!(fs::read(&a).unwrap(), b"after");
        fs::write(&a, b"external").unwrap();
        assert!(restore_batch(&p).is_err());
        fs::write(&a, b"after").unwrap();
        restore_batch(&p).unwrap();
        assert_eq!(fs::read(&a).unwrap(), b"before");
        assert!(!b.exists());
        let _ = fs::remove_dir_all(&p);
    }
}

/// Install a user-authored JSON skin with bounded size and strict schema validation.
pub fn install_skin_file(source: &Path, destination: &Path) -> Result<PathBuf> {
    let bytes = read_small(source)?.ok_or("Theme pack file not found")?;
    if bytes.len() > 65536 { return Err("Theme pack exceeds 64 KB".into()); }
    let profile: Value = serde_json::from_slice(&bytes).map_err(|e| format!("Theme pack JSON: {e}"))?;
    crate::skinpack::validate(&profile)?;
    install_with_backup(destination, serde_json::to_vec_pretty(&profile).map_err(|e| e.to_string())?)
}

/// One-click theme activation: workspace in FilmCraft's data directory + runtime profile.
/// Restores the workspace on a profile write failure. The stock binary ignores the runtime profile.
pub fn install_complete_theme(data_dir: &Path, skin_file: &Path, style: Style) -> Result<PathBuf> {
    let workspace = data_dir.join("workspaces.json");
    install_workspace(&workspace, style)?;
    match install_skin_config(skin_file, style) {
        Ok(backup) => Ok(backup),
        Err(error) => {
            let rollback = restore_latest(&workspace);
            if let Err(why) = rollback {
                Err(format!("Theme profile failed: {error}; workspace rollback failed: {why}"))
            } else {
                Err(format!("Theme profile failed (workspace rolled back): {error}"))
            }
        }
    }
}
