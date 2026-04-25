pub mod wire;

pub use codex_app_server_protocol as app_server_protocol;
pub use codex_protocol as protocol;

pub const PINNED_CODEX_CLI_VERSION: &str = "0.124.0";
pub const PINNED_CODEX_TAG: &str = "rust-v0.124.0";
pub const PINNED_CODEX_COMMIT: &str = "e9fb49366c93a1478ec71cc41ecee415a197d036";

pub fn pinned_codex_version_label() -> String {
    format!("{PINNED_CODEX_TAG}@{}", &PINNED_CODEX_COMMIT[..12])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_version_label_matches_expected_commit_prefix() {
        assert_eq!(
            pinned_codex_version_label(),
            "rust-v0.124.0@e9fb49366c93".to_string()
        );
    }
}
