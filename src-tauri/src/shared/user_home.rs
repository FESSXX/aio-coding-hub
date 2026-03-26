//! Usage: Resolve the effective user HOME directory consistently across runtime and tests.

#[cfg_attr(target_os = "windows", allow(dead_code))]
pub(crate) fn home_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::shared::error::AppResult<std::path::PathBuf> {
    crate::app_paths::home_dir(app)
}
