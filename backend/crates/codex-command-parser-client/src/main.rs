use std::{env, path::PathBuf, process::Command};

use anyhow::{Context, Result, bail};
use clap::Parser;
use reqwest::blocking::Client;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "codex-command-parser-client")]
struct Cli {
    #[arg(long = "request-additional", alias = "request-info")]
    additional_request: Option<String>,

    #[arg(long = "warnings")]
    include_warnings: bool,

    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParseRequest {
    command: Vec<String>,
    output: String,
    include_warnings: bool,
    additional_request: Option<String>,
    cwd: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cwd = env::current_dir().context("failed to resolve current directory")?;
    let outcome = Command::new(&cli.command[0])
        .args(&cli.command[1..])
        .current_dir(&cwd)
        .env("IS_USING_COMMAND_PARSER", "true")
        .output()
        .with_context(|| format!("failed to run {}", cli.command[0]))?;

    let mut output = String::new();
    output.push_str(&String::from_utf8_lossy(&outcome.stdout));
    output.push_str(&String::from_utf8_lossy(&outcome.stderr));

    let request = ParseRequest {
        command: cli.command,
        output,
        include_warnings: cli.include_warnings,
        additional_request: cli.additional_request.filter(|value| !value.trim().is_empty()),
        cwd: cwd.display().to_string(),
    };

    let base_url = env::var("CODEX_AUX_SERVER_NEW_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8871".to_string());
    let url = format!("{}/v1/command-parser/parse", base_url.trim_end_matches('/'));
    let client = Client::new();
    let response = client
        .post(url)
        .json(&request)
        .send()
        .context("failed to call command-parser server")?;

    let status = response.status();
    let body = response.text().context("failed to read command-parser response")?;
    if !status.is_success() {
        let message = body.trim();
        if message.is_empty() {
            bail!("command-parser-new server request failed with {}", status);
        }
        bail!("{message}");
    }

    print!("{}", body.trim_end());
    Ok(())
}
