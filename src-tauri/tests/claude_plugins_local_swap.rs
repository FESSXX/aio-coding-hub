mod support;

use std::fs;

#[test]
fn local_claude_plugins_are_stashed_and_restored_per_workspace_and_config_is_reset() {
    let app = support::TestApp::new();
    let handle = app.handle();

    let cli_root = app.home_dir().join(".claude").join("plugins");
    fs::create_dir_all(cli_root.join("repos")).expect("create plugins repos dir");
    fs::write(cli_root.join("repos").join("foo.txt"), "foo\n").expect("write foo plugin");
    fs::write(
        cli_root.join("config.json"),
        "{\"repositories\":{\"x\":1}}\n",
    )
    .expect("write non-default config");

    aio_coding_hub_lib::test_support::plugins_swap_local_for_workspace_switch(
        &handle,
        "claude",
        Some(1),
        2,
    )
    .expect("swap 1 -> 2");

    assert!(
        !cli_root.join("repos").join("foo.txt").exists(),
        "plugins should be isolated across workspaces"
    );
    assert!(
        cli_root.join("marketplaces").exists(),
        "marketplaces should exist"
    );
    assert!(cli_root.join("repos").exists(), "repos should exist");

    let config = fs::read_to_string(cli_root.join("config.json")).expect("read config.json");
    assert!(
        config.ends_with('\n'),
        "config.json should end with newline"
    );
    let value: serde_json::Value = serde_json::from_str(&config).expect("parse config.json");
    assert_eq!(value, serde_json::json!({ "repositories": {} }));

    let stash1 = app
        .home_dir()
        .join(app.app_dotdir_name())
        .join("plugins-local")
        .join("claude")
        .join("1")
        .join("plugins");
    assert!(
        stash1.join("repos").join("foo.txt").exists(),
        "stash 1 should have foo"
    );

    fs::write(cli_root.join("repos").join("bar.txt"), "bar\n").expect("write bar plugin");
    fs::write(
        cli_root.join("config.json"),
        "{\"repositories\":{\"y\":2}}\n",
    )
    .expect("mutate config.json");

    aio_coding_hub_lib::test_support::plugins_swap_local_for_workspace_switch(
        &handle,
        "claude",
        Some(2),
        1,
    )
    .expect("swap 2 -> 1");

    assert!(
        cli_root.join("repos").join("foo.txt").exists(),
        "foo should be restored for workspace 1"
    );
    assert!(
        !cli_root.join("repos").join("bar.txt").exists(),
        "bar should not leak into workspace 1"
    );

    let config = fs::read_to_string(cli_root.join("config.json")).expect("read config.json");
    let value: serde_json::Value = serde_json::from_str(&config).expect("parse config.json");
    assert_eq!(value, serde_json::json!({ "repositories": {} }));

    let stash2 = app
        .home_dir()
        .join(app.app_dotdir_name())
        .join("plugins-local")
        .join("claude")
        .join("2")
        .join("plugins");
    assert!(
        stash2.join("repos").join("bar.txt").exists(),
        "stash 2 should have bar"
    );
}
