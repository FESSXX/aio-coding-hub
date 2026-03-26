mod support;

#[test]
fn app_paths_and_db_init_are_isolated_under_home() {
    let app = support::TestApp::new();
    let handle = app.handle();

    let app_dir = aio_coding_hub_lib::test_support::app_data_dir(&handle).expect("app_data_dir");
    assert!(
        app_dir.ends_with(app.app_dotdir_name()),
        "app_dir should use the isolated test dotdir: app_dir={app_dir:?} dotdir={:?}",
        app.app_dotdir_name()
    );

    aio_coding_hub_lib::test_support::init_db(&handle).expect("init_db");

    let db_path = aio_coding_hub_lib::test_support::db_path(&handle).expect("db_path");
    assert!(db_path.exists(), "db file missing: db_path={db_path:?}");

    let conn = rusqlite::Connection::open(&db_path).expect("open db");
    let user_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read sqlite user_version");
    assert!(
        user_version > 0,
        "unexpected sqlite user_version={user_version}"
    );
}
