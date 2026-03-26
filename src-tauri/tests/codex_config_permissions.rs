mod support;

use std::ffi::OsString;

struct EnvVarRestore {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarRestore {
    fn set(key: &'static str, value: impl Into<OsString>) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value.into());
        Self { key, previous }
    }
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

#[test]
fn codex_config_get_allows_opening_followed_external_codex_home_directory() {
    let app = support::TestApp::new();
    let handle = app.handle();

    let follow_home = tempfile::tempdir().expect("follow home");
    let follow_codex_dir = follow_home.path().join("external-codex");
    std::fs::create_dir_all(&follow_codex_dir).expect("create external codex dir");

    let _env = EnvVarRestore::set("CODEX_HOME", &follow_codex_dir);

    let mut settings =
        aio_coding_hub_lib::test_support::settings_get_json(&handle).expect("read defaults");
    settings["codex_home_mode"] = serde_json::json!("follow_codex_home");
    settings["codex_home_override"] = serde_json::json!("");
    let _ = aio_coding_hub_lib::test_support::settings_set_json(&handle, settings).expect("write");

    let state =
        aio_coding_hub_lib::test_support::codex_config_get_json(&handle).expect("codex config");

    assert_eq!(
        state.get("config_dir").and_then(|value| value.as_str()),
        Some(follow_codex_dir.to_string_lossy().as_ref())
    );
    assert_eq!(
        state
            .get("follow_codex_home_dir")
            .and_then(|value| value.as_str()),
        Some(follow_codex_dir.to_string_lossy().as_ref())
    );
    assert_eq!(
        state
            .get("can_open_config_dir")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
}
