use robdex_agent_runtime::lifecycle;
use uuid::Uuid;

#[test]
fn terminal_update_errors_when_running_row_is_absent() {
    let missing_turn = Uuid::new_v4();
    let error = lifecycle::ensure_one_updated("turn", missing_turn, 0)
        .expect_err("missing/non-running turn must error");

    assert!(
        error
            .to_string()
            .contains("expected one running row, updated 0")
    );
}
