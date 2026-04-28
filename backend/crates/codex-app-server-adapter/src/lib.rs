pub mod wire;

pub use codex_app_server_protocol as app_server_protocol;
pub use codex_protocol as protocol;

pub const PINNED_CODEX_CLI_VERSION: &str = "0.125.0";
pub const PINNED_CODEX_TAG: &str = "rust-v0.125.0";
pub const PINNED_CODEX_COMMIT: &str = "637f7dd6d737f3961e6bf32fbb3861c4953269c5";

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
            "rust-v0.125.0@637f7dd6d737".to_string()
        );
    }
}
