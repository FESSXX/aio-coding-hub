mod support;

use serde_json::Value;

fn read_bytes(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).expect("read bytes")
}

fn read_json(path: &std::path::Path) -> Value {
    serde_json::from_slice(&read_bytes(path)).expect("parse json")
}

#[test]
fn claude_proxy_enable_preserves_existing_settings_fields() {
    let app = support::TestApp::new();
    let handle = app.handle();
    let settings_path = app.home_dir().join(".claude").join("settings.json");

    std::fs::create_dir_all(settings_path.parent().expect("settings dir")).expect("create dir");
    std::fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "model": "claude-3-7-sonnet",
            "outputStyle": "concise",
            "permissions": {
                "allow": ["Bash(ls:*)"]
            },
            "env": {
                "KEEP_ME": "1",
                "MCP_TIMEOUT": "120000"
            }
        }))
        .expect("serialize settings"),
    )
    .expect("write settings");

    let result = aio_coding_hub_lib::test_support::cli_proxy_set_enabled_json(
        &handle,
        "claude",
        true,
        "http://127.0.0.1:37123",
    )
    .expect("enable claude proxy");

    assert!(result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    let current = read_json(&settings_path);
    assert_eq!(
        current.get("model").and_then(|v| v.as_str()),
        Some("claude-3-7-sonnet")
    );
    assert_eq!(
        current.get("outputStyle").and_then(|v| v.as_str()),
        Some("concise")
    );
    assert_eq!(
        current
            .get("permissions")
            .and_then(|v| v.get("allow"))
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .and_then(|v| v.as_str()),
        Some("Bash(ls:*)")
    );
    assert_eq!(
        current
            .get("env")
            .and_then(|v| v.get("KEEP_ME"))
            .and_then(|v| v.as_str()),
        Some("1")
    );
    assert_eq!(
        current
            .get("env")
            .and_then(|v| v.get("MCP_TIMEOUT"))
            .and_then(|v| v.as_str()),
        Some("120000")
    );
    assert_eq!(
        current
            .get("env")
            .and_then(|v| v.get("ANTHROPIC_BASE_URL"))
            .and_then(|v| v.as_str()),
        Some("http://127.0.0.1:37123/claude")
    );
    assert_eq!(
        current
            .get("env")
            .and_then(|v| v.get("ANTHROPIC_AUTH_TOKEN"))
            .and_then(|v| v.as_str()),
        Some("aio-coding-hub")
    );
}

#[test]
fn claude_proxy_disable_restores_original_settings_bytes() {
    let app = support::TestApp::new();
    let handle = app.handle();
    let settings_path = app.home_dir().join(".claude").join("settings.json");

    std::fs::create_dir_all(settings_path.parent().expect("settings dir")).expect("create dir");

    let original_json: Value = serde_json::from_str(
        r#"{
  "model": "claude-opus-4",
  "env": {
    "KEEP_ME": "x",
    "ANTHROPIC_BASE_URL": "https://example.com",
    "ANTHROPIC_AUTH_TOKEN": "sk-real"
  },
  "permissions": {
    "deny": ["Bash(rm:*)"]
  }
}"#,
    )
    .expect("parse original");
    std::fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&original_json).expect("serialize"),
    )
    .expect("write settings");

    let enabled = aio_coding_hub_lib::test_support::cli_proxy_set_enabled_json(
        &handle,
        "claude",
        true,
        "http://127.0.0.1:37123",
    )
    .expect("enable claude proxy");
    assert!(enabled.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));

    let disabled = aio_coding_hub_lib::test_support::cli_proxy_set_enabled_json(
        &handle,
        "claude",
        false,
        "http://127.0.0.1:37123",
    )
    .expect("disable claude proxy");
    assert!(disabled
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false));

    // Compare JSON values (merge-restore may reorder keys but preserves content)
    let restored = read_json(&settings_path);
    assert_eq!(restored, original_json);
}

#[test]
fn claude_proxy_disable_restores_direct_env_after_settings_patch() {
    let app = support::TestApp::new();
    let handle = app.handle();
    let settings_path = app.home_dir().join(".claude").join("settings.json");

    std::fs::create_dir_all(settings_path.parent().expect("settings dir")).expect("create dir");
    std::fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://direct.example.com",
                "ANTHROPIC_AUTH_TOKEN": "sk-direct"
            }
        }))
        .expect("serialize settings"),
    )
    .expect("write settings");

    aio_coding_hub_lib::test_support::cli_proxy_set_enabled_json(
        &handle,
        "claude",
        true,
        "http://127.0.0.1:37123",
    )
    .expect("enable claude proxy");

    aio_coding_hub_lib::test_support::cli_manager_claude_settings_set_json(
        &handle,
        serde_json::json!({
            "always_thinking_enabled": true
        }),
    )
    .expect("patch claude settings");

    aio_coding_hub_lib::test_support::cli_proxy_set_enabled_json(
        &handle,
        "claude",
        false,
        "http://127.0.0.1:37123",
    )
    .expect("disable claude proxy");

    let restored = read_json(&settings_path);
    assert_eq!(
        restored
            .get("alwaysThinkingEnabled")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        restored
            .get("env")
            .and_then(|v| v.get("ANTHROPIC_BASE_URL"))
            .and_then(|v| v.as_str()),
        Some("https://direct.example.com")
    );
    assert_eq!(
        restored
            .get("env")
            .and_then(|v| v.get("ANTHROPIC_AUTH_TOKEN"))
            .and_then(|v| v.as_str()),
        Some("sk-direct")
    );
}

#[test]
fn claude_proxy_disable_preserves_direct_settings_created_while_enabled() {
    let app = support::TestApp::new();
    let handle = app.handle();
    let settings_path = app.home_dir().join(".claude").join("settings.json");

    aio_coding_hub_lib::test_support::cli_proxy_set_enabled_json(
        &handle,
        "claude",
        true,
        "http://127.0.0.1:37123",
    )
    .expect("enable claude proxy");

    aio_coding_hub_lib::test_support::cli_manager_claude_settings_set_json(
        &handle,
        serde_json::json!({
            "always_thinking_enabled": true
        }),
    )
    .expect("patch claude settings");

    aio_coding_hub_lib::test_support::cli_manager_claude_env_set_json(&handle, Some(90_000), true)
        .expect("patch claude env");

    aio_coding_hub_lib::test_support::cli_proxy_set_enabled_json(
        &handle,
        "claude",
        false,
        "http://127.0.0.1:37123",
    )
    .expect("disable claude proxy");

    let restored = read_json(&settings_path);
    assert_eq!(
        restored
            .get("alwaysThinkingEnabled")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        restored
            .get("env")
            .and_then(|v| v.get("MCP_TIMEOUT"))
            .and_then(|v| v.as_str()),
        Some("90000")
    );
    assert_eq!(
        restored
            .get("env")
            .and_then(|v| v.get("DISABLE_ERROR_REPORTING"))
            .and_then(|v| v.as_str()),
        Some("1")
    );
    assert!(
        restored
            .get("env")
            .and_then(|v| v.get("ANTHROPIC_BASE_URL"))
            .is_none(),
        "{restored}"
    );
    assert!(
        restored
            .get("env")
            .and_then(|v| v.get("ANTHROPIC_AUTH_TOKEN"))
            .is_none(),
        "{restored}"
    );
}
