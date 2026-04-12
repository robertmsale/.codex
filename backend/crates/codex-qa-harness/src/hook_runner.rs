use std::{collections::BTreeMap, path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    time::timeout,
};

use crate::models::HookResult;

#[derive(Debug, Clone)]
pub struct HookRunRequest {
    pub program: PathBuf,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub env: BTreeMap<String, String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct HookRunOutput {
    pub result: HookResult,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HookRunner;

impl HookRunner {
    pub async fn run(&self, request: HookRunRequest) -> Result<HookRunOutput> {
        let mut command = Command::new(&request.program);
        command
            .current_dir(&request.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &request.env {
            command.env(key, value);
        }

        let mut child = command.spawn().with_context(|| {
            format!(
                "spawn hook {} in {}",
                request.program.display(),
                request.cwd.display()
            )
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            let payload = serde_json::to_vec(&request.payload)?;
            stdin.write_all(&payload).await?;
            stdin.shutdown().await?;
        }

        let output = match timeout(request.timeout, child.wait_with_output()).await {
            Ok(result) => result.with_context(|| {
                format!(
                    "wait for hook {} in {}",
                    request.program.display(),
                    request.cwd.display()
                )
            })?,
            Err(_) => bail!(
                "hook {} timed out after {}s",
                request.program.display(),
                request.timeout.as_secs()
            ),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !output.status.success() {
            bail!(
                "hook {} failed with exit code {:?}: {}",
                request.program.display(),
                output.status.code(),
                first_nonempty_line(&stderr).or_else(|| first_nonempty_line(&stdout)).unwrap_or("no output")
            );
        }
        if stdout.is_empty() {
            bail!("hook {} returned empty stdout", request.program.display());
        }

        let result: HookResult = serde_json::from_str(&stdout).with_context(|| {
            format!(
                "parse hook stdout as json for {}: {}",
                request.program.display(),
                stdout
            )
        })?;
        if !result.ok {
            let message = result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .or(result.message.as_deref())
                .unwrap_or("hook returned ok=false");
            return Err(anyhow!(message.to_string()));
        }

        Ok(HookRunOutput {
            result,
            stdout,
            stderr,
        })
    }
}

fn first_nonempty_line(value: &str) -> Option<&str> {
    value.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn hook_runner_passes_json_stdin_and_env_and_parses_result() {
        let temp = tempdir().expect("tempdir");
        let script = temp.path().join("hook.sh");
        fs::write(
            &script,
            r#"#!/bin/bash
read payload
echo "{\"ok\":true,\"artifacts\":{\"seen_payload_len\":${#payload},\"seen_env\":\"$QAH_TEST_ENV\"}}"
"#,
        )
        .expect("write script");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms).expect("chmod");
        }

        let mut env = BTreeMap::new();
        env.insert("QAH_TEST_ENV".to_string(), "present".to_string());

        let output = HookRunner
            .run(HookRunRequest {
                program: script,
                cwd: temp.path().to_path_buf(),
                timeout: Duration::from_secs(2),
                env,
                payload: serde_json::json!({"hello":"world"}),
            })
            .await
            .expect("hook should succeed");

        assert_eq!(
            output.result.artifacts.get("seen_env"),
            Some(&serde_json::json!("present"))
        );
        assert_eq!(
            output.result.artifacts.get("seen_payload_len"),
            Some(&serde_json::json!(17))
        );
    }

    #[tokio::test]
    async fn hook_runner_rejects_ok_false_result() {
        let temp = tempdir().expect("tempdir");
        let script = temp.path().join("hook.sh");
        fs::write(
            &script,
            r#"#!/bin/bash
echo '{"ok":false,"error":{"code":"bad","message":"not ready"}}'
"#,
        )
        .expect("write script");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms).expect("chmod");
        }

        let error = HookRunner
            .run(HookRunRequest {
                program: script,
                cwd: temp.path().to_path_buf(),
                timeout: Duration::from_secs(2),
                env: BTreeMap::new(),
                payload: serde_json::json!({}),
            })
            .await
            .expect_err("ok=false should fail");

        assert!(error.to_string().contains("not ready"));
    }
}
