use robdex_agent_runtime::{db, lifecycle};
use uuid::Uuid;

const DEFAULT_DATABASE_URL: &str =
    "postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime";

#[tokio::test]
async fn terminal_update_errors_when_running_row_is_absent() {
    let database_url = std::env::var("ROBDEX_AGENT_RUNTIME_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    let pool = db::connect(&database_url).await.expect("connect postgres");
    db::init(&pool).await.expect("init schema");

    let missing_turn = Uuid::new_v4();
    let error = lifecycle::complete_turn(
        &pool,
        missing_turn,
        lifecycle::TerminalStatus::Completed,
        chrono::Utc::now(),
    )
    .await
    .expect_err("missing/non-running turn must error");

    assert!(
        error
            .to_string()
            .contains("expected one running row, updated 0")
    );
}
