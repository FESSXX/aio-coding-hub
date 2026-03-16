use super::*;

#[test]
fn codex_proxy_preserves_nested_model_provider_tables_and_order() {
    let input = r#"
model_provider = "aio"
preferred_auth_method = "apikey"

[model_providers.aio]
name = "aio"
base_url = "http://old/v1"
wire_api = "responses"
requires_openai_auth = true

[model_providers.aio.projects."C:\\work"]
trust_level = "trusted"

[other]
foo = "bar"
"#;

    let out =
        build_codex_config_toml(Some(input.as_bytes().to_vec()), "http://new/v1").expect("build");
    let s = String::from_utf8(out).expect("utf8");

    assert!(s.contains("base_url = \"http://new/v1\""), "{s}");
    assert!(
        s.contains("[model_providers.aio.projects.\"C:\\\\work\"]"),
        "{s}"
    );
    assert!(s.contains("trust_level = \"trusted\""), "{s}");

    let base_idx = s.find("[model_providers.aio]").expect("base table exists");
    let nested_idx = s
        .find("[model_providers.aio.projects.\"C:\\\\work\"]")
        .expect("nested table exists");
    assert!(base_idx < nested_idx, "base must appear before nested: {s}");
}

#[test]
fn codex_proxy_preserves_extra_keys_in_base_table() {
    let input = r#"
[model_providers.aio]
name = "aio"
base_url = "http://old/v1"
wire_api = "responses"
requires_openai_auth = true
trusted_roots = ["C:\\work"]
"#;

    let out =
        build_codex_config_toml(Some(input.as_bytes().to_vec()), "http://new/v1").expect("build");
    let s = String::from_utf8(out).expect("utf8");

    assert!(s.contains("base_url = \"http://new/v1\""), "{s}");
    assert!(s.contains("trusted_roots = [\"C:\\\\work\"]"), "{s}");
}

#[test]
fn codex_proxy_dedupes_multiple_base_tables() {
    let input = r#"
[model_providers."aio"]
base_url = "http://old-1/v1"

[model_providers.aio]
base_url = "http://old-2/v1"

[model_providers.aio.projects."C:\\work"]
trust_level = "trusted"
"#;

    let out =
        build_codex_config_toml(Some(input.as_bytes().to_vec()), "http://new/v1").expect("build");
    let s = String::from_utf8(out).expect("utf8");

    let count = s.matches("[model_providers.aio]").count()
        + s.matches("[model_providers.\"aio\"]").count()
        + s.matches("[model_providers.'aio']").count();
    assert_eq!(count, 1, "{s}");
    assert!(s.contains("base_url = \"http://new/v1\""), "{s}");
    assert!(
        s.contains("[model_providers.aio.projects.\"C:\\\\work\"]"),
        "{s}"
    );
}

#[test]
fn codex_proxy_inserts_base_table_before_nested_when_missing() {
    let input = r#"
[model_providers.aio.projects."C:\\work"]
trust_level = "trusted"
"#;

    let out =
        build_codex_config_toml(Some(input.as_bytes().to_vec()), "http://new/v1").expect("build");
    let s = String::from_utf8(out).expect("utf8");

    let base_idx = s
        .find("[model_providers.aio]")
        .expect("base table inserted");
    let nested_idx = s
        .find("[model_providers.aio.projects.\"C:\\\\work\"]")
        .expect("nested table exists");
    assert!(base_idx < nested_idx, "base must appear before nested: {s}");
}

#[test]
fn codex_proxy_moves_base_table_before_nested_when_out_of_order() {
    let input = r#"
[model_providers.aio.projects."C:\\work"]
trust_level = "trusted"

[model_providers.aio]
name = "aio"
base_url = "http://old/v1"
wire_api = "responses"
requires_openai_auth = true
"#;

    let out =
        build_codex_config_toml(Some(input.as_bytes().to_vec()), "http://new/v1").expect("build");
    let s = String::from_utf8(out).expect("utf8");

    let base_idx = s.find("[model_providers.aio]").expect("base table exists");
    let nested_idx = s
        .find("[model_providers.aio.projects.\"C:\\\\work\"]")
        .expect("nested table exists");
    assert!(base_idx < nested_idx, "base must appear before nested: {s}");
}

#[test]
fn codex_proxy_auth_json_preserves_existing_oauth_fields() {
    let input = r#"{
  "oauth_access_token": "tok-123",
  "oauth_refresh_token": "ref-456",
  "OPENAI_API_KEY": "old-key"
}"#;

    let out = build_codex_auth_json(Some(input.as_bytes().to_vec())).expect("build auth");
    let value: serde_json::Value = serde_json::from_slice(&out).expect("parse output");

    assert_eq!(
        value.get("OPENAI_API_KEY").and_then(|v| v.as_str()),
        Some("aio-coding-hub")
    );
    assert_eq!(
        value.get("oauth_access_token").and_then(|v| v.as_str()),
        Some("tok-123")
    );
    assert_eq!(
        value.get("oauth_refresh_token").and_then(|v| v.as_str()),
        Some("ref-456")
    );
}

#[test]
fn codex_proxy_auth_json_rejects_non_object_root() {
    let input = r#"["not", "an", "object"]"#;
    let err = build_codex_auth_json(Some(input.as_bytes().to_vec())).expect_err("must fail");
    assert!(err
        .to_string()
        .contains("auth.json root must be a JSON object"));
}

#[test]
fn claude_proxy_settings_json_rejects_invalid_json() {
    let input = br#"{"env": "#.to_vec();
    let err = build_claude_settings_json(Some(input), "http://127.0.0.1:1717/claude")
        .expect_err("must fail");
    assert!(err.to_string().contains("CLI_PROXY_INVALID_SETTINGS_JSON"));
}

// ── merge-restore tests ─────────────────────────────────────────────────────

fn write_temp(dir: &std::path::Path, name: &str, content: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

#[test]
fn merge_restore_claude_preserves_user_changes() {
    let tmp = tempfile::tempdir().unwrap();

    // Backup: original file before proxy was enabled
    let backup = write_temp(
        tmp.path(),
        "backup.json",
        br#"{ "model": "opus", "permissions": { "allow": ["Read"] } }"#,
    );

    // Current: user added "language" and proxy injected env keys
    let target = write_temp(
        tmp.path(),
        "settings.json",
        br#"{
  "model": "opus",
  "language": "zh-CN",
  "permissions": { "allow": ["Read", "Write"] },
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:37123/claude",
    "ANTHROPIC_AUTH_TOKEN": "aio-coding-hub",
    "MCP_TIMEOUT": "30000"
  }
}"#,
    );

    merge_restore_claude_settings_json(&target, &backup).unwrap();

    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();

    // User's added "language" is preserved
    assert_eq!(result.get("language").unwrap().as_str(), Some("zh-CN"));
    // User's updated permissions are preserved
    let allow = result["permissions"]["allow"].as_array().unwrap();
    assert_eq!(allow.len(), 2);
    // Proxy keys are removed (backup didn't have them)
    let env = result.get("env").unwrap().as_object().unwrap();
    assert!(!env.contains_key("ANTHROPIC_BASE_URL"));
    assert!(!env.contains_key("ANTHROPIC_AUTH_TOKEN"));
    // User's other env keys are preserved
    assert_eq!(env.get("MCP_TIMEOUT").unwrap().as_str(), Some("30000"));
}

#[test]
fn merge_restore_claude_restores_original_env_keys() {
    let tmp = tempfile::tempdir().unwrap();

    // Backup: had original ANTHROPIC_BASE_URL
    let backup = write_temp(
        tmp.path(),
        "backup.json",
        br#"{ "env": { "ANTHROPIC_BASE_URL": "https://api.anthropic.com" } }"#,
    );

    // Current: proxy replaced the URL
    let target = write_temp(
        tmp.path(),
        "settings.json",
        br#"{ "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:37123/claude", "ANTHROPIC_AUTH_TOKEN": "aio" } }"#,
    );

    merge_restore_claude_settings_json(&target, &backup).unwrap();

    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    let env = result.get("env").unwrap().as_object().unwrap();
    assert_eq!(
        env.get("ANTHROPIC_BASE_URL").unwrap().as_str(),
        Some("https://api.anthropic.com")
    );
    assert!(!env.contains_key("ANTHROPIC_AUTH_TOKEN"));
}

#[test]
fn merge_restore_codex_auth_preserves_user_changes() {
    let tmp = tempfile::tempdir().unwrap();

    // Backup: had OAuth tokens
    let backup = write_temp(
        tmp.path(),
        "backup.json",
        br#"{ "tokens": { "access": "tok-123" }, "last_refresh": 1234, "custom_key": "keep" }"#,
    );

    // Current: proxy replaced auth, user added a new key
    let target = write_temp(
        tmp.path(),
        "auth.json",
        br#"{ "OPENAI_API_KEY": "aio-coding-hub", "auth_mode": "apikey", "user_added": "hello" }"#,
    );

    merge_restore_codex_auth_json(&target, &backup).unwrap();

    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    // Proxy keys removed
    assert!(result.get("OPENAI_API_KEY").is_none());
    assert!(result.get("auth_mode").is_none());
    // OAuth tokens restored from backup
    assert!(result.get("tokens").is_some());
    assert!(result.get("last_refresh").is_some());
    // User's addition preserved
    assert_eq!(result.get("user_added").unwrap().as_str(), Some("hello"));
}

#[test]
fn merge_restore_gemini_env_preserves_user_changes() {
    let tmp = tempfile::tempdir().unwrap();

    // Backup: had original API key
    let backup = write_temp(
        tmp.path(),
        "backup.env",
        b"GEMINI_API_KEY=original-key\nCUSTOM_VAR=keep\n",
    );

    // Current: proxy replaced keys, user added new var
    let target = write_temp(
        tmp.path(),
        ".env",
        b"GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:37123/gemini\nGEMINI_API_KEY=aio-coding-hub\nUSER_VAR=hello\n",
    );

    merge_restore_gemini_env(&target, &backup).unwrap();

    let result = std::fs::read_to_string(&target).unwrap();
    // Proxy base URL removed (backup didn't have it)
    assert!(!result.contains("GOOGLE_GEMINI_BASE_URL"));
    // Original API key restored
    assert!(result.contains("GEMINI_API_KEY=original-key"));
    // User's addition preserved
    assert!(result.contains("USER_VAR=hello"));
}

#[test]
fn merge_restore_codex_config_preserves_user_changes() {
    let tmp = tempfile::tempdir().unwrap();

    // Backup: no proxy config, just user settings
    let backup = write_temp(
        tmp.path(),
        "backup.toml",
        b"[model_providers.openai]\nname = \"openai\"\nbase_url = \"https://api.openai.com/v1\"\n",
    );

    // Current: proxy added its config, user added a new section
    let target = write_temp(
        tmp.path(),
        "config.toml",
        b"model_provider = \"aio\"\npreferred_auth_method = \"apikey\"\n\n[model_providers.openai]\nname = \"openai\"\nbase_url = \"https://api.openai.com/v1\"\n\n[model_providers.aio]\nname = \"aio\"\nbase_url = \"http://127.0.0.1:37123/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n\n[user_section]\nfoo = \"bar\"\n\n[windows]\nsandbox = \"elevated\"\n",
    );

    merge_restore_codex_config_toml(&target, &backup).unwrap();

    let result = std::fs::read_to_string(&target).unwrap();
    // Proxy root keys removed (check for the root-level assignment, not table names)
    assert!(
        !result.contains("model_provider = \"aio\""),
        "model_provider root key should be removed: {result}"
    );
    assert!(
        !result.contains("preferred_auth_method"),
        "preferred_auth_method should be removed: {result}"
    );
    // Proxy provider section removed
    assert!(!result.contains("[model_providers.aio]"));
    // Proxy windows sandbox removed
    assert!(!result.contains("[windows]"));
    assert!(!result.contains("sandbox"));
    // User's openai section preserved
    assert!(result.contains("[model_providers.openai]"));
    assert!(result.contains("base_url = \"https://api.openai.com/v1\""));
    // User's custom section preserved
    assert!(result.contains("[user_section]"));
    assert!(result.contains("foo = \"bar\""));
}
