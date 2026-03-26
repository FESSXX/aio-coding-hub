//! Usage: Stash / restore Claude Code local `plugins` per workspace.

use crate::app_paths;
use crate::shared::fs::write_file_atomic_if_changed;
use crate::shared::time::now_unix_seconds;
use std::path::{Path, PathBuf};

fn validate_cli_key(cli_key: &str) -> crate::shared::error::AppResult<()> {
    crate::shared::cli_key::validate_cli_key(cli_key)?;
    if cli_key == "claude" {
        Ok(())
    } else {
        Err(
            format!("SEC_INVALID_INPUT: claude plugins swap only supports cli_key={cli_key}")
                .into(),
        )
    }
}

fn home_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::shared::error::AppResult<PathBuf> {
    crate::app_paths::home_dir(app)
}

fn claude_plugins_root<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::shared::error::AppResult<PathBuf> {
    Ok(home_dir(app)?.join(".claude").join("plugins"))
}

fn stash_bucket_name(workspace_id: Option<i64>) -> String {
    workspace_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unassigned".to_string())
}

fn stash_root<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    cli_key: &str,
) -> crate::shared::error::AppResult<PathBuf> {
    Ok(app_paths::app_data_dir(app)?
        .join("plugins-local")
        .join(cli_key))
}

use crate::shared::fs::is_symlink;

fn rotate_existing_dir(dst: &Path) -> crate::shared::error::AppResult<()> {
    if !dst.exists() {
        return Ok(());
    }
    let Some(parent) = dst.parent() else {
        return Ok(());
    };
    let base = dst
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("plugins")
        .to_string();

    let nonce = now_unix_seconds();
    let mut candidate = parent.join(format!(".{base}.old-{nonce}"));
    let mut idx = 2;
    while candidate.exists() && idx < 100 {
        candidate = parent.join(format!(".{base}.old-{nonce}-{idx}"));
        idx += 1;
    }

    std::fs::rename(dst, &candidate)
        .map_err(|e| format!("failed to rotate {}: {e}", dst.display()))?;
    Ok(())
}

fn move_dir(src: &Path, dst: &Path) -> crate::shared::error::AppResult<()> {
    let Some(parent) = dst.parent() else {
        return Err(format!("SEC_INVALID_INPUT: invalid dst path {}", dst.display()).into());
    };
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;

    if dst.exists() {
        rotate_existing_dir(dst)?;
    }

    std::fs::rename(src, dst)
        .map_err(|e| format!("failed to move {} -> {}: {e}", src.display(), dst.display()).into())
}

fn ensure_clean_plugins_layout(plugins_root: &Path) -> crate::shared::error::AppResult<()> {
    if plugins_root.exists() && !plugins_root.is_dir() {
        return Err(format!(
            "SEC_INVALID_INPUT: plugins path exists but is not a directory: {}",
            plugins_root.display()
        )
        .into());
    }

    std::fs::create_dir_all(plugins_root.join("marketplaces")).map_err(|e| {
        format!(
            "failed to create marketplaces dir {}: {e}",
            plugins_root.display()
        )
    })?;
    std::fs::create_dir_all(plugins_root.join("repos"))
        .map_err(|e| format!("failed to create repos dir {}: {e}", plugins_root.display()))?;

    let config_path = plugins_root.join("config.json");
    if config_path.exists() && is_symlink(&config_path)? {
        return Err(format!(
            "SEC_INVALID_INPUT: refusing to modify symlink path={}",
            config_path.display()
        )
        .into());
    }

    let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({ "repositories": {} }))
        .map_err(|e| format!("failed to serialize plugins config json: {e}"))?;
    bytes.push(b'\n');
    let _ = write_file_atomic_if_changed(&config_path, &bytes)?;
    Ok(())
}

#[derive(Debug)]
pub(crate) struct LocalPluginsSwap {
    cli_root: PathBuf,
    from_bucket_plugins: PathBuf,
    to_bucket_plugins: PathBuf,

    had_cli_root: bool,
}

impl LocalPluginsSwap {
    pub(crate) fn rollback(self) {
        // Best-effort: restore previous state by stashing current cli root back to the to-bucket,
        // then restoring the from-bucket if it existed.
        if self.cli_root.exists() {
            let _ = move_dir(&self.cli_root, &self.to_bucket_plugins);
        }

        if self.had_cli_root && self.from_bucket_plugins.exists() {
            let _ = move_dir(&self.from_bucket_plugins, &self.cli_root);
        }
    }
}

pub(crate) fn swap_local_plugins_for_workspace_switch<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    cli_key: &str,
    from_workspace_id: Option<i64>,
    to_workspace_id: i64,
) -> crate::shared::error::AppResult<LocalPluginsSwap> {
    validate_cli_key(cli_key)?;

    let cli_root = claude_plugins_root(app)?;
    if cli_root.exists() && is_symlink(&cli_root)? {
        return Err(format!(
            "SEC_INVALID_INPUT: refusing to modify symlink path={}",
            cli_root.display()
        )
        .into());
    }
    if cli_root.exists() && !cli_root.is_dir() {
        return Err(format!(
            "SEC_INVALID_INPUT: plugins path exists but is not a directory: {}",
            cli_root.display()
        )
        .into());
    }

    let stash_root = stash_root(app, cli_key)?;
    let from_bucket = stash_root.join(stash_bucket_name(from_workspace_id));
    let to_bucket = stash_root.join(to_workspace_id.to_string());

    std::fs::create_dir_all(&from_bucket)
        .map_err(|e| format!("failed to create {}: {e}", from_bucket.display()))?;
    std::fs::create_dir_all(&to_bucket)
        .map_err(|e| format!("failed to create {}: {e}", to_bucket.display()))?;

    let from_bucket_plugins = from_bucket.join("plugins");
    let to_bucket_plugins = to_bucket.join("plugins");

    let had_cli_root = cli_root.exists();

    let swap = LocalPluginsSwap {
        cli_root: cli_root.clone(),
        from_bucket_plugins: from_bucket_plugins.clone(),
        to_bucket_plugins: to_bucket_plugins.clone(),
        had_cli_root,
    };

    if had_cli_root {
        move_dir(&cli_root, &from_bucket_plugins).map_err(|err| {
            format!("CLAUDE_PLUGINS_SWAP_FAILED: failed to stash plugins dir: {err}")
        })?;
    }

    if to_bucket_plugins.exists() {
        if let Err(err) = move_dir(&to_bucket_plugins, &cli_root) {
            swap.rollback();
            return Err(format!(
                "CLAUDE_PLUGINS_SWAP_FAILED: failed to restore plugins dir: {err}"
            )
            .into());
        }
    } else if !cli_root.exists() {
        if let Err(err) = std::fs::create_dir_all(&cli_root)
            .map_err(|e| format!("failed to create {}: {e}", cli_root.display()))
        {
            swap.rollback();
            return Err(
                format!("CLAUDE_PLUGINS_SWAP_FAILED: failed to create plugins dir: {err}").into(),
            );
        }
    }

    if let Err(err) = ensure_clean_plugins_layout(&cli_root) {
        swap.rollback();
        return Err(format!(
            "CLAUDE_PLUGINS_SWAP_FAILED: failed to seed clean plugins layout: {err}"
        )
        .into());
    }

    Ok(swap)
}
