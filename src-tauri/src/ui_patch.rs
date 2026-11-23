use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};
use tauri::{Assets, Runtime};
use zip::ZipArchive;

use crate::voice::setup::app_data_root;

pub const UI_MANIFEST_URL: &str =
    "https://github.com/zigglyzuff/undetect/releases/download/ui-latest/ui-manifest.json";

/// Ed25519 verifying key matching the CI `UI_PATCH_SIGNING_PRIVATE_KEY` secret.
pub const UI_PATCH_PUBLIC_KEY: [u8; 32] = [
    0x87, 0x8c, 0x76, 0x07, 0x84, 0xfa, 0x68, 0x90,
    0x2d, 0xdb, 0xc6, 0xfd, 0x9b, 0x04, 0xd1, 0xdc,
    0xb6, 0x54, 0x7b, 0x2f, 0xff, 0x8f, 0xc2, 0xfd,
    0xb5, 0x95, 0x6c, 0x63, 0xcb, 0x2f, 0x3f, 0xa9,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiPatchMeta {
    pub ui_version: String,
    pub min_app_version: String,
    pub max_app_version: String,
    pub sha256: String,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub boot_attempts: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiPatchState {
    pub last_binary_version: String,
    pub active: Option<UiPatchMeta>,
    pub pending: Option<UiPatchMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPatchManifest {
    pub ui_version: String,
    pub min_app_version: String,
    pub max_app_version: String,
    pub sha256: String,
    pub url: String,
    #[serde(default)]
    pub pub_date: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiPatchSyncResult {
    pub staged: bool,
    pub active_ui_version: Option<String>,
    pub pending_ui_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActiveUiPatch {
    pub ui_version: String,
    pub bundle_dir: PathBuf,
}

pub fn ui_patches_root() -> PathBuf {
    app_data_root().join("ui_patches")
}

pub fn ui_patches_root_in(base: &Path) -> PathBuf {
    base.join("ui_patches")
}

fn state_path(root: &Path) -> PathBuf {
    root.join("state.json")
}

fn bundles_dir(root: &Path) -> PathBuf {
    root.join("bundles")
}

fn bundle_dir_for(root: &Path, ui_version: &str) -> PathBuf {
    bundles_dir(root).join(sanitize_version_dir(ui_version))
}

fn staging_dir(root: &Path) -> PathBuf {
    root.join("staging")
}

fn sanitize_version_dir(version: &str) -> String {
    let cleaned: String = version
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "patch".to_string()
    } else {
        cleaned
    }
}

pub fn load_patch_state(root: &Path) -> UiPatchState {
    let path = state_path(root);
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<UiPatchState>(&raw).ok())
        .unwrap_or_default()
}

pub fn save_patch_state(root: &Path, state: &UiPatchState) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| format!("Failed to create ui_patches dir: {e}"))?;
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize ui_patches state: {e}"))?;
    let tmp = root.join("state.json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("Failed to write temp state: {e}"))?;
    let dest = state_path(root);
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    fs::rename(&tmp, &dest).map_err(|e| format!("Failed to persist ui_patches state: {e}"))
}

pub fn canonical_manifest_message(manifest: &UiPatchManifest) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        manifest.ui_version,
        manifest.min_app_version,
        manifest.max_app_version,
        manifest.sha256,
        manifest.url
    )
}

pub fn default_verifying_key() -> Result<VerifyingKey, String> {
    VerifyingKey::from_bytes(&UI_PATCH_PUBLIC_KEY)
        .map_err(|e| format!("Invalid embedded UI patch public key: {e}"))
}

pub fn verify_manifest_signature(
    manifest: &UiPatchManifest,
    verifying_key: &VerifyingKey,
) -> Result<(), String> {
    let sig_bytes = B64
        .decode(manifest.signature.trim())
        .map_err(|e| format!("Invalid manifest base64 signature: {e}"))?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| "Invalid Ed25519 signature byte length".to_string())?;
    let signature = Signature::from_bytes(&sig_arr);
    let message = canonical_manifest_message(manifest);
    verifying_key
        .verify(message.as_bytes(), &signature)
        .map_err(|e| format!("UI patch manifest signature verification failed: {e}"))
}

fn parse_semver_bound(raw: &str, wildcard_fill: u64) -> Option<(u64, u64, u64)> {
    let base = raw.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = base.split('.');
    let parse_part = |s: Option<&str>, default_val: u64| -> Option<u64> {
        match s.map(str::trim) {
            None => Some(default_val),
            Some("x") | Some("*") => Some(wildcard_fill),
            Some(num) => num.parse::<u64>().ok(),
        }
    };
    let major = parse_part(parts.next(), 0)?;
    let minor = parse_part(parts.next(), 0)?;
    let patch = parse_part(parts.next(), 0)?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn app_version_compatible(current_app: &str, min_app: &str, max_app: &str) -> bool {
    let Some(cur) = parse_semver_bound(current_app, 0) else {
        return false;
    };
    let Some(min_v) = parse_semver_bound(min_app, 0) else {
        return false;
    };
    let Some(max_v) = parse_semver_bound(max_app, u64::MAX) else {
        return false;
    };
    cur >= min_v && cur <= max_v
}

/// Resolves and advances startup UI patch state inside `root`.
/// Handles binary-version invalidation, pending -> active promotion, and boot crash rollback.
pub fn resolve_startup_patch_in(root: &Path, current_app_version: &str) -> Option<ActiveUiPatch> {
    let mut state = load_patch_state(root);

    // Always drop older UI patches after a full binary upgrade so fresh embedded UI is never shadowed.
    if !state.last_binary_version.is_empty() && state.last_binary_version != current_app_version {
        let _ = fs::remove_dir_all(bundles_dir(root));
        let _ = fs::remove_dir_all(staging_dir(root));
        state = UiPatchState {
            last_binary_version: current_app_version.to_string(),
            active: None,
            pending: None,
        };
        let _ = save_patch_state(root, &state);
        return None;
    }

    if state.last_binary_version.is_empty() {
        state.last_binary_version = current_app_version.to_string();
    }

    let mut promoted_now = false;
    if let Some(mut pending) = state.pending.take() {
        let pending_dir = bundle_dir_for(root, &pending.ui_version);
        if app_version_compatible(
            current_app_version,
            &pending.min_app_version,
            &pending.max_app_version,
        ) && pending_dir.join("index.html").is_file()
        {
            pending.confirmed = false;
            pending.boot_attempts = 1;
            state.active = Some(pending);
            promoted_now = true;
            prune_unused_bundles(root, state.active.as_ref(), None);
        }
    }

    if let Some(active) = state.active.as_mut() {
        let active_dir = bundle_dir_for(root, &active.ui_version);
        let compatible = app_version_compatible(
            current_app_version,
            &active.min_app_version,
            &active.max_app_version,
        );
        if !compatible || !active_dir.join("index.html").is_file() {
            state.active = None;
            prune_unused_bundles(root, None, state.pending.as_ref());
            let _ = save_patch_state(root, &state);
            return None;
        }

        if !promoted_now {
            active.boot_attempts = active.boot_attempts.saturating_add(1);
        }

        let max_attempts = if active.confirmed { 3 } else { 2 };
        if active.boot_attempts >= max_attempts {
            eprintln!(
                "[Undetect] Rolling back UI patch {} after {} unconfirmed boot(s).",
                active.ui_version, active.boot_attempts
            );
            crate::telemetry::record_ui_patch_rollback(
                &active.ui_version,
                active.boot_attempts,
                current_app_version,
            );
            state.active = None;
            prune_unused_bundles(root, None, state.pending.as_ref());
            let _ = save_patch_state(root, &state);
            return None;
        }
    }

    let result = state.active.as_ref().map(|meta| ActiveUiPatch {
        ui_version: meta.ui_version.clone(),
        bundle_dir: bundle_dir_for(root, &meta.ui_version),
    });
    let _ = save_patch_state(root, &state);
    result
}

pub fn resolve_startup_patch(current_app_version: &str) -> Option<ActiveUiPatch> {
    if cfg!(debug_assertions) && std::env::var("UNDETECT_TEST_UI_PATCH").is_err() {
        return None;
    }
    resolve_startup_patch_in(&ui_patches_root(), current_app_version)
}

pub fn confirm_ui_boot_in(root: &Path) -> Result<(), String> {
    let mut state = load_patch_state(root);
    if let Some(active) = state.active.as_mut()
        && (!active.confirmed || active.boot_attempts != 0)
    {
        active.confirmed = true;
        active.boot_attempts = 0;
        save_patch_state(root, &state)?;
    }
    Ok(())
}

pub fn confirm_ui_boot() -> Result<(), String> {
    if cfg!(debug_assertions) && std::env::var("UNDETECT_TEST_UI_PATCH").is_err() {
        return Ok(());
    }
    confirm_ui_boot_in(&ui_patches_root())
}

fn prune_unused_bundles(
    root: &Path,
    active: Option<&UiPatchMeta>,
    pending: Option<&UiPatchMeta>,
) {
    let dir = bundles_dir(root);
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    let active_name = active.map(|a| sanitize_version_dir(&a.ui_version));
    let pending_name = pending.map(|p| sanitize_version_dir(&p.ui_version));

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if active_name.as_deref() != Some(name.as_str())
            && pending_name.as_deref() != Some(name.as_str())
        {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

pub fn verify_and_extract_zip(
    zip_bytes: &[u8],
    expected_sha256: &str,
    dest_dir: &Path,
) -> Result<(), String> {
    let actual_hex = sha256_hex(zip_bytes);
    let expected_norm = expected_sha256.trim().to_ascii_lowercase();
    if actual_hex != expected_norm {
        return Err(format!(
            "UI patch SHA-256 mismatch (expected {expected_norm}, got {actual_hex})"
        ));
    }

    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir)
            .map_err(|e| format!("Failed to clean staging folder: {e}"))?;
    }
    fs::create_dir_all(dest_dir).map_err(|e| format!("Failed to create staging folder: {e}"))?;

    let cursor = io::Cursor::new(zip_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| format!("Failed to open UI patch zip: {e}"))?;

    for idx in 0..archive.len() {
        let mut file = archive
            .by_index(idx)
            .map_err(|e| format!("Failed to read zip entry {idx}: {e}"))?;
        let Some(rel_path) = file.enclosed_name() else {
            return Err("Rejected UI patch zip containing unsafe path traversal".to_string());
        };
        let out_path = dest_dir.join(&rel_path);
        if file.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create dir {}: {e}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create dir {}: {e}", parent.display()))?;
            }
            let mut out_file = fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create file {}: {e}", out_path.display()))?;
            io::copy(&mut file, &mut out_file)
                .map_err(|e| format!("Failed to write file {}: {e}", out_path.display()))?;
        }
    }

    if !dest_dir.join("index.html").is_file() {
        return Err("Extracted UI patch missing index.html".to_string());
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn sync_ui_patch_with(
    root: &Path,
    current_app_version: &str,
    manifest_url: &str,
    verifying_key: &VerifyingKey,
) -> Result<UiPatchSyncResult, String> {
    let client = reqwest::Client::builder()
        .user_agent("Undetect-App")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to initialize UI patch HTTP client: {e}"))?;

    let manifest_res = client
        .get(manifest_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch UI patch manifest: {e}"))?;

    if !manifest_res.status().is_success() {
        return Err(format!(
            "UI patch manifest returned HTTP {}",
            manifest_res.status()
        ));
    }

    let manifest: UiPatchManifest = manifest_res
        .json()
        .await
        .map_err(|e| format!("Invalid UI patch manifest JSON: {e}"))?;

    verify_manifest_signature(&manifest, verifying_key)?;

    if !manifest.url.starts_with("https://") && !manifest.url.starts_with("http://127.0.0.1:") {
        return Err("Rejected non-HTTPS UI patch download URL".to_string());
    }

    let mut state = load_patch_state(root);
    let active_ver = state.active.as_ref().map(|a| a.ui_version.clone());
    let pending_ver = state.pending.as_ref().map(|p| p.ui_version.clone());

    if !app_version_compatible(
        current_app_version,
        &manifest.min_app_version,
        &manifest.max_app_version,
    ) {
        return Ok(UiPatchSyncResult {
            staged: false,
            active_ui_version: active_ver,
            pending_ui_version: pending_ver,
        });
    }

    if active_ver.as_deref() == Some(manifest.ui_version.as_str())
        || pending_ver.as_deref() == Some(manifest.ui_version.as_str())
    {
        return Ok(UiPatchSyncResult {
            staged: false,
            active_ui_version: active_ver,
            pending_ui_version: pending_ver,
        });
    }

    let zip_res = client
        .get(&manifest.url)
        .send()
        .await
        .map_err(|e| format!("Failed to download UI patch bundle: {e}"))?;

    if !zip_res.status().is_success() {
        return Err(format!(
            "UI patch bundle returned HTTP {}",
            zip_res.status()
        ));
    }

    let zip_bytes = zip_res
        .bytes()
        .await
        .map_err(|e| format!("Failed to read UI patch bundle bytes: {e}"))?;

    let stage_path = staging_dir(root).join(sanitize_version_dir(&manifest.ui_version));
    verify_and_extract_zip(&zip_bytes, &manifest.sha256, &stage_path)?;

    let final_bundle_dir = bundle_dir_for(root, &manifest.ui_version);
    if let Some(parent) = final_bundle_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create bundles directory: {e}"))?;
    }
    if final_bundle_dir.exists() {
        fs::remove_dir_all(&final_bundle_dir)
            .map_err(|e| format!("Failed to clear prior UI bundle dir: {e}"))?;
    }
    fs::rename(&stage_path, &final_bundle_dir)
        .map_err(|e| format!("Failed to promote staged UI bundle: {e}"))?;

    let meta = UiPatchMeta {
        ui_version: manifest.ui_version.clone(),
        min_app_version: manifest.min_app_version,
        max_app_version: manifest.max_app_version,
        sha256: manifest.sha256,
        confirmed: false,
        boot_attempts: 0,
    };
    state.last_binary_version = current_app_version.to_string();
    state.pending = Some(meta);
    prune_unused_bundles(root, state.active.as_ref(), state.pending.as_ref());
    save_patch_state(root, &state)?;

    Ok(UiPatchSyncResult {
        staged: true,
        active_ui_version: state.active.map(|a| a.ui_version),
        pending_ui_version: Some(manifest.ui_version),
    })
}

pub async fn sync_ui_patch(current_app_version: &str) -> Result<UiPatchSyncResult, String> {
    if cfg!(debug_assertions) && std::env::var("UNDETECT_TEST_UI_PATCH").is_err() {
        return Ok(UiPatchSyncResult {
            staged: false,
            active_ui_version: None,
            pending_ui_version: None,
        });
    }
    let manifest_url = std::env::var("UNDETECT_UI_MANIFEST_URL")
        .unwrap_or_else(|_| UI_MANIFEST_URL.to_string());
    let key = default_verifying_key()?;
    sync_ui_patch_with(&ui_patches_root(), current_app_version, &manifest_url, &key).await
}

pub fn sanitize_asset_rel_path(asset_key: &str) -> Option<PathBuf> {
    let trimmed = asset_key.trim_start_matches('/');
    let rel_str = if trimmed.is_empty() {
        "index.html"
    } else {
        trimmed
    };
    if rel_str.contains('\\') || rel_str.contains(':') {
        return None;
    }
    let mut out = PathBuf::new();
    for comp in Path::new(rel_str).components() {
        match comp {
            Component::Normal(seg) => out.push(seg),
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Asset provider layered over compiled `EmbeddedAssets`.
/// Serves verified UI hot-patch files from disk when present and falls back cleanly otherwise.
pub struct HotSwapAssets<R: Runtime> {
    bundle_dir: PathBuf,
    fallback: Box<dyn Assets<R>>,
}

impl<R: Runtime> HotSwapAssets<R> {
    pub fn new(bundle_dir: PathBuf, fallback: Box<dyn Assets<R>>) -> Self {
        Self {
            bundle_dir,
            fallback,
        }
    }
}

impl<R: Runtime> Assets<R> for HotSwapAssets<R> {
    fn setup(&self, app: &tauri::App<R>) {
        self.fallback.setup(app);
    }

    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        if let Some(rel) = sanitize_asset_rel_path(key.as_ref()) {
            let candidate = self.bundle_dir.join(rel);
            if candidate.is_file()
                && let Ok(bytes) = fs::read(candidate)
            {
                return Some(Cow::Owned(bytes));
            }
        }
        self.fallback.get(key)
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        self.fallback.iter()
    }

    fn csp_hashes(&self, html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        self.fallback.csp_hashes(html_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn temp_root(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "undetect_ui_patch_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(io::Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default();
            for &(name, data) in entries {
                zip.start_file(name, opts).unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn verifies_valid_ed25519_manifest_and_rejects_tampering() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let verifying = signing.verifying_key();

        let mut manifest = UiPatchManifest {
            ui_version: "2.2.0-ui.1".to_string(),
            min_app_version: "2.2.0".to_string(),
            max_app_version: "2.2.x".to_string(),
            sha256: "abc123".to_string(),
            url: "https://github.com/zigglyzuff/undetect/releases/download/ui-latest/dist.zip"
                .to_string(),
            pub_date: "2026-11-11T00:00:00Z".to_string(),
            signature: String::new(),
        };
        let msg = canonical_manifest_message(&manifest);
        let sig = signing.sign(msg.as_bytes());
        manifest.signature = B64.encode(sig.to_bytes());

        assert!(verify_manifest_signature(&manifest, &verifying).is_ok());

        let mut tampered = manifest.clone();
        tampered.sha256 = "def456".to_string();
        assert!(verify_manifest_signature(&tampered, &verifying).is_err());
    }

    #[test]
    fn validates_semver_compatibility_bounds() {
        assert!(app_version_compatible("2.2.0", "2.2.0", "2.2.0"));
        assert!(app_version_compatible("2.2.3", "2.2.0", "2.2.x"));
        assert!(!app_version_compatible("2.3.0", "2.2.0", "2.2.x"));
        assert!(!app_version_compatible("2.1.0", "2.2.0", "2.2.x"));
    }

    #[test]
    fn extracts_valid_zip_and_rejects_path_traversal_and_bad_hash() {
        let root = temp_root("zip");
        let dest = root.join("out");

        let valid_zip = make_zip(&[
            ("index.html", b"<html>ok</html>"),
            ("assets/app.js", b"console.log(1);"),
        ]);
        let hash = sha256_hex(&valid_zip);

        assert!(verify_and_extract_zip(&valid_zip, &"0".repeat(64), &dest).is_err());
        assert!(verify_and_extract_zip(&valid_zip, &hash, &dest).is_ok());
        assert!(dest.join("index.html").is_file());
        assert!(dest.join("assets").join("app.js").is_file());

        let traversal_zip = make_zip(&[("../evil.txt", b"pwn")]);
        let bad_hash = sha256_hex(&traversal_zip);
        assert!(verify_and_extract_zip(&traversal_zip, &bad_hash, &root.join("bad")).is_err());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn promotes_pending_patch_and_rolls_back_on_unconfirmed_restarts() {
        let root = temp_root("rollback");
        let bdir = bundle_dir_for(&root, "2.2.0-ui.1");
        fs::create_dir_all(&bdir).unwrap();
        fs::write(bdir.join("index.html"), b"<html>patched</html>").unwrap();

        save_patch_state(
            &root,
            &UiPatchState {
                last_binary_version: "2.2.0".to_string(),
                active: None,
                pending: Some(UiPatchMeta {
                    ui_version: "2.2.0-ui.1".to_string(),
                    min_app_version: "2.2.0".to_string(),
                    max_app_version: "2.2.0".to_string(),
                    sha256: "deadbeef".to_string(),
                    confirmed: false,
                    boot_attempts: 0,
                }),
            },
        )
        .unwrap();

        // Launch 1 promotes pending -> active and starts trial.
        let launch1 = resolve_startup_patch_in(&root, "2.2.0");
        assert_eq!(
            launch1.as_ref().map(|p| p.ui_version.as_str()),
            Some("2.2.0-ui.1")
        );

        // Launch 2 without confirm_ui_boot rolls back cleanly to embedded assets.
        let launch2 = resolve_startup_patch_in(&root, "2.2.0");
        assert!(launch2.is_none());
        assert!(load_patch_state(&root).active.is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn confirms_healthy_boot_and_purges_on_binary_upgrade() {
        let root = temp_root("upgrade");
        let bdir = bundle_dir_for(&root, "2.2.0-ui.2");
        fs::create_dir_all(&bdir).unwrap();
        fs::write(bdir.join("index.html"), b"<html>patched2</html>").unwrap();

        save_patch_state(
            &root,
            &UiPatchState {
                last_binary_version: "2.2.0".to_string(),
                active: None,
                pending: Some(UiPatchMeta {
                    ui_version: "2.2.0-ui.2".to_string(),
                    min_app_version: "2.2.0".to_string(),
                    max_app_version: "2.2.x".to_string(),
                    sha256: "abc".to_string(),
                    confirmed: false,
                    boot_attempts: 0,
                }),
            },
        )
        .unwrap();

        assert!(resolve_startup_patch_in(&root, "2.2.0").is_some());
        confirm_ui_boot_in(&root).unwrap();
        assert!(resolve_startup_patch_in(&root, "2.2.0").is_some());
        confirm_ui_boot_in(&root).unwrap();

        // Binary upgrade invalidates old patch bundles immediately.
        assert!(resolve_startup_patch_in(&root, "2.3.0").is_none());
        assert!(!bdir.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn hotswap_assets_serves_patched_file_and_falls_back_to_embedded() {
        struct StubAssets;
        impl Assets<tauri::Wry> for StubAssets {
            fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
                if key.as_ref() == "/assets/logo.png" {
                    Some(Cow::Borrowed(b"embedded-logo"))
                } else if key.as_ref() == "/index.html" {
                    Some(Cow::Borrowed(b"embedded-index"))
                } else {
                    None
                }
            }
            fn iter(&self) -> Box<AssetsIter<'_>> {
                Box::new(std::iter::empty())
            }
            fn csp_hashes(&self, _: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
                Box::new(std::iter::empty())
            }
        }

        let root = temp_root("assets");
        fs::write(root.join("index.html"), b"patched-index").unwrap();

        let wrapped = HotSwapAssets::new(root.clone(), Box::new(StubAssets));
        assert_eq!(
            wrapped.get(&AssetKey::from("index.html")).as_deref(),
            Some(&b"patched-index"[..])
        );
        assert_eq!(
            wrapped.get(&AssetKey::from("/assets/logo.png")).as_deref(),
            Some(&b"embedded-logo"[..])
        );
        assert!(wrapped.get(&AssetKey::from("/../secret")).is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn syncs_signed_patch_over_http_and_stages_for_next_boot() {
        use axum::{routing::get, Json, Router};

        let root = temp_root("http_sync");
        let signing = SigningKey::from_bytes(&[9u8; 32]);
        let verifying = signing.verifying_key();

        let zip_bytes = make_zip(&[
            ("index.html", b"<html>synced-ui</html>"),
            ("viewer.html", b"<html>synced-viewer</html>"),
            ("assets/main.js", b"console.log('synced');"),
        ]);
        let sha = sha256_hex(&zip_bytes);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let zip_url = format!("http://{addr}/dist.zip");
        let manifest_url = format!("http://{addr}/ui-manifest.json");

        let mut manifest = UiPatchManifest {
            ui_version: "2.2.0-ui.99".to_string(),
            min_app_version: "2.2.0".to_string(),
            max_app_version: "2.2.x".to_string(),
            sha256: sha,
            url: zip_url,
            pub_date: "2026-11-11T00:00:00Z".to_string(),
            signature: String::new(),
        };
        let sig = signing.sign(canonical_manifest_message(&manifest).as_bytes());
        manifest.signature = B64.encode(sig.to_bytes());

        let manifest_clone = manifest.clone();
        let zip_clone = zip_bytes.clone();
        let router = Router::new()
            .route(
                "/ui-manifest.json",
                get(move || {
                    let m = manifest_clone.clone();
                    async move { Json(m) }
                }),
            )
            .route(
                "/dist.zip",
                get(move || {
                    let z = zip_clone.clone();
                    async move { z }
                }),
            );

        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        let synced = sync_ui_patch_with(&root, "2.2.0", &manifest_url, &verifying)
            .await
            .unwrap();
        assert!(synced.staged);
        assert_eq!(synced.pending_ui_version.as_deref(), Some("2.2.0-ui.99"));

        // Re-checking the same UI version is a no-op.
        let second = sync_ui_patch_with(&root, "2.2.0", &manifest_url, &verifying)
            .await
            .unwrap();
        assert!(!second.staged);

        // Next boot promotes the staged patch and serves its index.html / viewer.html.
        let active = resolve_startup_patch_in(&root, "2.2.0").unwrap();
        assert_eq!(active.ui_version, "2.2.0-ui.99");
        assert_eq!(
            fs::read(active.bundle_dir.join("viewer.html")).unwrap(),
            b"<html>synced-viewer</html>"
        );

        server.abort();
        let _ = fs::remove_dir_all(&root);
    }
}
