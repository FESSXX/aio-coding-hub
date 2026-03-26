mod support;

use std::fs;

#[test]
fn local_skills_are_stashed_and_restored_per_workspace() {
    let app = support::TestApp::new();
    let handle = app.handle();

    let cli_root = app.home_dir().join(".claude").join("skills");
    fs::create_dir_all(&cli_root).expect("create cli skills root");

    let managed_dir = cli_root.join("managed-skill");
    fs::create_dir_all(&managed_dir).expect("create managed skill dir");
    fs::write(
        managed_dir.join(".aio-coding-hub.managed"),
        "aio-coding-hub\n",
    )
    .expect("write managed marker");
    fs::write(managed_dir.join("SKILL.md"), "name: Managed\n").expect("write managed SKILL.md");

    let local_dir = cli_root.join("local-one");
    fs::create_dir_all(&local_dir).expect("create local skill dir");
    fs::write(local_dir.join("SKILL.md"), "name: Local One\n").expect("write local SKILL.md");

    let other_dir = cli_root.join("random-dir");
    fs::create_dir_all(&other_dir).expect("create unrelated dir");

    aio_coding_hub_lib::test_support::skills_swap_local_for_workspace_switch(
        &handle,
        "claude",
        Some(1),
        2,
    )
    .expect("swap 1 -> 2");

    assert!(
        !local_dir.exists(),
        "local skill should be moved out of cli root"
    );
    assert!(managed_dir.exists(), "managed skill should remain");
    assert!(other_dir.exists(), "unrelated dir should remain");

    let stash_dir = app
        .home_dir()
        .join(app.app_dotdir_name())
        .join("skills-local")
        .join("claude")
        .join("1")
        .join("local-one");
    assert!(stash_dir.exists(), "stash dir should exist");

    aio_coding_hub_lib::test_support::skills_swap_local_for_workspace_switch(
        &handle,
        "claude",
        Some(2),
        1,
    )
    .expect("swap 2 -> 1");

    assert!(
        local_dir.exists(),
        "local skill should be restored to cli root"
    );
    assert!(
        managed_dir.exists(),
        "managed skill should remain after restore"
    );
}
