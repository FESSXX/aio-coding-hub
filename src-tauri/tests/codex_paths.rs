mod support;

#[test]
fn codex_paths_use_user_home_default_by_default() {
    let app = support::TestApp::new();
    let handle = app.handle();

    // Ensure no leftover CODEX_HOME from other tests
    std::env::remove_var("CODEX_HOME");

    let path =
        aio_coding_hub_lib::test_support::codex_config_toml_path(&handle).expect("default path");
    assert_eq!(
        path,
        app.home_dir().join(".codex").join("config.toml"),
        "default mode should resolve to ~/.codex/config.toml"
    );
}

#[test]
fn codex_paths_follow_codex_home_env_when_mode_is_follow_codex_home() {
    let app = support::TestApp::new();
    let handle = app.handle();

    let mut settings =
        aio_coding_hub_lib::test_support::settings_get_json(&handle).expect("read defaults");
    settings["codex_home_mode"] = serde_json::json!("follow_codex_home");
    let _ = aio_coding_hub_lib::test_support::settings_set_json(&handle, settings).expect("write");

    std::env::set_var("CODEX_HOME", "codex-home");
    let path = aio_coding_hub_lib::test_support::codex_config_toml_path(&handle)
        .expect("relative CODEX_HOME");
    assert_eq!(path, app.home_dir().join("codex-home").join("config.toml"));

    std::env::set_var("CODEX_HOME", "~/.codex-alt");
    let path =
        aio_coding_hub_lib::test_support::codex_config_toml_path(&handle).expect("tilde expand");
    assert_eq!(path, app.home_dir().join(".codex-alt").join("config.toml"));

    let abs_dir = app.home_dir().join("abs-codex");
    std::env::set_var("CODEX_HOME", abs_dir.as_os_str());
    let path =
        aio_coding_hub_lib::test_support::codex_config_toml_path(&handle).expect("absolute path");
    assert_eq!(path, abs_dir.join("config.toml"));

    // Clean up env var to avoid polluting other tests
    std::env::remove_var("CODEX_HOME");
}

#[test]
fn codex_paths_prefers_settings_override_and_normalizes_config_toml_input() {
    let app = support::TestApp::new();
    let handle = app.handle();

    // 即使存在 CODEX_HOME，自定义模式也应优先使用 settings 中的 override。
    std::env::set_var("CODEX_HOME", "env-codex-home");

    let mut settings =
        aio_coding_hub_lib::test_support::settings_get_json(&handle).expect("read defaults");
    settings["codex_home_mode"] = serde_json::json!("custom");
    settings["codex_home_override"] =
        serde_json::json!(app.home_dir().join("custom-codex").join("config.toml"));
    let _ = aio_coding_hub_lib::test_support::settings_set_json(&handle, settings).expect("write");

    let path = aio_coding_hub_lib::test_support::codex_config_toml_path(&handle)
        .expect("settings override path");
    assert_eq!(
        path,
        app.home_dir().join("custom-codex").join("config.toml")
    );

    std::env::remove_var("CODEX_HOME");
}

#[test]
fn codex_follow_env_or_default_ignores_settings_override() {
    let app = support::TestApp::new();
    let handle = app.handle();

    // Clean env first
    std::env::remove_var("CODEX_HOME");

    let mut settings =
        aio_coding_hub_lib::test_support::settings_get_json(&handle).expect("read defaults");
    settings["codex_home_mode"] = serde_json::json!("custom");
    settings["codex_home_override"] = serde_json::json!(app.home_dir().join("custom-codex"));
    let _ = aio_coding_hub_lib::test_support::settings_set_json(&handle, settings).expect("write");

    let path = aio_coding_hub_lib::test_support::codex_home_dir_follow_env_or_default(&handle)
        .expect("follow default path");
    assert_eq!(path, app.home_dir().join(".codex"));

    std::env::set_var("CODEX_HOME", "env-codex-home");
    let path = aio_coding_hub_lib::test_support::codex_home_dir_follow_env_or_default(&handle)
        .expect("follow env path");
    assert_eq!(path, app.home_dir().join("env-codex-home"));

    // Clean up
    std::env::remove_var("CODEX_HOME");
}
