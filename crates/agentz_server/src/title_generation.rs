//! Thread titles written by a coding agent's CLI, as t3code's text generation writes them
//! (`apps/server/src/textGeneration`): its prompt, its output schema, and each CLI run the way
//! t3code runs it, with nothing of the user's checkout. The setting is kept in
//! `title-generation.json` in the data directory.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use agentz_protocol::title_generation::{
    ANTIGRAVITY_DEFAULT_MODEL, TitleGeneration, TitleModel, TitleProvider, TitleProviderInfo,
};
use anyhow::{Context as _, Result, anyhow};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;

use crate::agent_settings::{read_json, write_json};

/// t3code's timeout for a CLI's text generation.
const GENERATION_TIMEOUT: Duration = Duration::from_secs(180);
/// How long `codex debug models` may take to list its models.
const MODELS_TIMEOUT: Duration = Duration::from_secs(20);
/// t3code's budget for the user's message in the prompt.
const MESSAGE_BUDGET: usize = 8_000;
const TRUNCATED: &str = "\n[Content truncated]\n";
/// The prompt asks for under 40 characters; this only stops a runaway model.
const MAX_TITLE_CHARS: usize = 120;

/// t3code's `INITIAL_THREAD_TITLE_PROMPT`, about agentZ.
const TITLE_PROMPT: &str = "Generate a title that will help the user recognize this agentZ thread weeks later.
Return JSON with keys title and needsRefinement.
Set needsRefinement to true only if the subject is still unknown, such as an unresolved link, \"fix this\", or an unexplained attachment. Otherwise set it to false.

Before answering, silently reduce the request to:
- Subject: What system, feature, or problem is this really about?
- Outcome: What does the user ultimately want to understand or change?
- Incidental instructions: What only describes how the agent should do the work?

Title the subject and outcome. Discard incidental instructions.

Editorial rules:
- 3-8 words, fewer than 40 characters.
- Use a compact noun phrase or clear action phrase.
- Capture the umbrella goal when the request lists several symptoms or steps.
- Name the product change, not the mock, plan, report, branch, or PR used to produce it.
- Models, subagents, tools, output formats, and monitoring instructions do not belong in the title unless they are themselves the topic.
- For reviews, name what is being reviewed and the relevant concern. Avoid generic titles such as \"Review PR 123\" when linked or attached context reveals the subject.
- For research, name the question domain rather than the requested research process.
- Do not claim the work is complete.
- Do not copy and truncate the user's message.
- Avoid project names already visible in the UI, quotes, labels, filler, and trailing punctuation.
- Use attached images as primary context for UI issues.
- When a URL or attachment is the only source of the subject, use available tools to inspect it directly.
- Local git history is not evidence of what a linked PR or issue is about. Never title the thread after branch names, commit messages, or merged commits found in the checkout.
- If a linked PR or issue cannot be read, fall back to the user's stated action plus its number, such as \"Take Over PR 8588\". This is the one case where a PR or issue number belongs in the title.";

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("title-generation.json")
}

pub(crate) fn load(data_dir: &Path) -> Result<TitleGeneration> {
    Ok(read_json(&path(data_dir))?.unwrap_or_default())
}

pub(crate) fn save(data_dir: &Path, settings: &TitleGeneration) -> Result<()> {
    write_json(&path(data_dir), settings)
}

/// Every provider, with whether its CLI is on `search_path` (the server's `PATH` if `None`)
/// and the models it offers: Codex's as `codex` lists them, the others' from t3code's list.
pub(crate) async fn find_providers(search_path: Option<OsString>) -> Vec<TitleProviderInfo> {
    let mut providers = Vec::new();
    for provider in TitleProvider::ALL {
        let program = find_program(&provider, search_path.as_ref());
        let models = match (&provider, &program) {
            (TitleProvider::Codex, Some(program)) => {
                codex_models(program, search_path.as_ref()).await
            }
            (TitleProvider::Codex, None) => vec![default_codex_model()],
            (TitleProvider::Claude, _) => claude_models(),
            _ => antigravity_models(),
        };
        providers.push(TitleProviderInfo {
            provider,
            installed: program.is_some(),
            models,
        });
    }
    providers
}

fn find_program(provider: &TitleProvider, search_path: Option<&OsString>) -> Option<PathBuf> {
    let name = provider.program()?;
    let search_path = search_path.cloned().or_else(|| std::env::var_os("PATH"))?;
    std::env::split_paths(&search_path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn effort_list(efforts: &[&str]) -> Vec<String> {
    efforts.iter().map(|effort| effort.to_string()).collect()
}

fn default_codex_model() -> TitleModel {
    TitleModel {
        id: TitleProvider::Codex.default_model().to_string(),
        name: "GPT-6-Luna".to_string(),
        efforts: effort_list(&["low", "medium", "high", "xhigh", "max"]),
    }
}

/// The models `codex debug models` lists for its picker, as t3code reads them from Codex's
/// `model/list`, with t3code's default first if Codex doesn't list it.
async fn codex_models(program: &Path, search_path: Option<&OsString>) -> Vec<TitleModel> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(["debug", "models"])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    if let Some(search_path) = search_path {
        command.env("PATH", search_path);
    }
    let listed = match tokio::time::timeout(MODELS_TIMEOUT, command.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            parse_codex_models(&output.stdout).unwrap_or_default()
        }
        Ok(Ok(output)) => {
            log::warn!(
                "codex debug models failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            Vec::new()
        }
        Ok(Err(error)) => {
            log::warn!("couldn't run codex debug models: {error}");
            Vec::new()
        }
        Err(_) => {
            log::warn!("codex debug models timed out");
            Vec::new()
        }
    };
    let mut models = listed;
    if !models
        .iter()
        .any(|model| model.id == TitleProvider::Codex.default_model())
    {
        models.insert(0, default_codex_model());
    }
    models
}

fn parse_codex_models(output: &[u8]) -> Option<Vec<TitleModel>> {
    let value: Value = serde_json::from_slice(output).ok()?;
    let models = value["models"].as_array()?;
    Some(
        models
            .iter()
            .filter(|model| model["visibility"].as_str() == Some("list"))
            .filter_map(|model| {
                let id = model["slug"].as_str()?.to_string();
                let name = model["display_name"].as_str().unwrap_or(&id).to_string();
                let efforts = model["supported_reasoning_levels"]
                    .as_array()
                    .map(|levels| {
                        levels
                            .iter()
                            .filter_map(|level| level["effort"].as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                Some(TitleModel { id, name, efforts })
            })
            .collect(),
    )
}

/// t3code's current Claude models, with their efforts (its prompt-injected ones left out),
/// and Haiku 4.5, its text generation default, which takes none.
fn claude_models() -> Vec<TitleModel> {
    let efforts = effort_list(&["low", "medium", "high", "xhigh", "max"]);
    let mut models: Vec<TitleModel> = [
        ("claude-fable-5-1", "Claude Fable 5.1"),
        ("claude-opus-5-5", "Claude Opus 5.5"),
        ("claude-sonnet-5-5", "Claude Sonnet 5.5"),
        ("claude-haiku-5-5", "Claude Haiku 5.5"),
    ]
    .into_iter()
    .map(|(id, name)| TitleModel {
        id: id.to_string(),
        name: name.to_string(),
        efforts: efforts.clone(),
    })
    .collect();
    models.push(TitleModel {
        id: TitleProvider::Claude.default_model().to_string(),
        name: "Claude Haiku 4.5".to_string(),
        efforts: Vec::new(),
    });
    models
}

/// t3code's Antigravity models, whose names carry their effort, after the CLI's own.
fn antigravity_models() -> Vec<TitleModel> {
    [
        (ANTIGRAVITY_DEFAULT_MODEL, "Default"),
        ("gemini-3.8-flash-high", "Gemini 3.8 Flash (High)"),
        ("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium)"),
        ("gemini-3.8-flash-low", "Gemini 3.8 Flash (Low)"),
    ]
    .into_iter()
    .map(|(id, name)| TitleModel {
        id: id.to_string(),
        name: name.to_string(),
        efforts: Vec::new(),
    })
    .collect()
}

/// t3code's `limitTitleMessage`: a long message keeps its start and its end.
fn limit_message(text: &str, budget: usize) -> String {
    let length = text.chars().count();
    if length <= budget {
        return text.to_string();
    }
    let available = budget - TRUNCATED.len();
    let head = available.div_ceil(2);
    let tail = available - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(length - tail).collect();
    format!("{start}{TRUNCATED}{end}")
}

pub(crate) fn prompt(message: &str) -> String {
    format!(
        "{TITLE_PROMPT}\n\nUser message:\n{}",
        limit_message(message, MESSAGE_BUDGET)
    )
}

fn output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "needsRefinement": {"type": "boolean"},
        },
        "required": ["title", "needsRefinement"],
        "additionalProperties": false,
    })
}

/// t3code's `sanitizeThreadTitle`: one line, without quotes around it, and not too long.
/// `None` when nothing is left.
pub(crate) fn sanitize_title(raw: &str) -> Option<String> {
    let decoded = serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value["title"].as_str().map(str::to_string));
    let title = decoded.as_deref().unwrap_or(raw);
    let line = title.trim().lines().next().unwrap_or_default().trim();
    let unquoted = line
        .trim_matches(|character| matches!(character, '\'' | '"' | '`'))
        .trim();
    let normalized = unquoted.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    if normalized.chars().count() <= MAX_TITLE_CHARS {
        return Some(normalized);
    }
    let cut: String = normalized.chars().take(MAX_TITLE_CHARS - 3).collect();
    Some(format!("{}...", cut.trim_end()))
}

/// A title for a thread whose first message is `message`, from the chosen CLI on
/// `search_path` (the server's `PATH` if `None`).
pub(crate) async fn generate(
    settings: TitleGeneration,
    message: String,
    search_path: Option<OsString>,
) -> Result<String> {
    let program = find_program(&settings.provider, search_path.as_ref()).ok_or_else(|| {
        anyhow!(
            "{} isn't on the PATH",
            settings.provider.program().unwrap_or("its CLI")
        )
    })?;
    // Titles need only the prompt, not anything of the checkout.
    let directory = std::env::temp_dir().join(format!("agentz-title-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    let result = run(&settings, &program, &directory, &prompt(&message), search_path).await;
    if let Err(error) = std::fs::remove_dir_all(&directory) {
        log::warn!("couldn't remove {}: {error}", directory.display());
    }
    let raw = result?;
    sanitize_title(&raw).ok_or_else(|| anyhow!("the title was empty"))
}

/// Runs the provider's CLI as t3code does, and returns the title it wrote, as written.
async fn run(
    settings: &TitleGeneration,
    program: &Path,
    directory: &Path,
    prompt: &str,
    search_path: Option<OsString>,
) -> Result<String> {
    let schema = output_schema().to_string();
    let mut command = tokio::process::Command::new(program);
    command.current_dir(directory);
    if let Some(search_path) = search_path {
        command.env("PATH", search_path);
    }
    let output_path = directory.join("output.json");
    let stdin = match &settings.provider {
        TitleProvider::Codex => {
            let schema_path = directory.join("schema.json");
            std::fs::write(&schema_path, &schema)
                .with_context(|| format!("writing {}", schema_path.display()))?;
            command
                .args(["exec", "--ephemeral", "--skip-git-repo-check", "-s", "read-only"])
                .args(["--model", settings.model()])
                .arg("--config")
                .arg(format!(
                    "model_reasoning_effort=\"{}\"",
                    settings.effort().unwrap_or("low")
                ))
                .arg("--output-schema")
                .arg(&schema_path)
                .arg("--output-last-message")
                .arg(&output_path)
                .arg("-");
            Some(prompt)
        }
        TitleProvider::Claude => {
            command
                .args(["-p", "--output-format", "json", "--json-schema", &schema])
                .args(["--model", settings.model()]);
            if let Some(effort) = settings.effort() {
                command.args(["--effort", effort]);
            }
            command
                .args(["--settings", r#"{"disableAllHooks":true}"#])
                .args(["--tools", ""])
                .args(["--disable-slash-commands", "--strict-mcp-config"])
                .args(["--permission-mode", "dontAsk"]);
            Some(prompt)
        }
        TitleProvider::Antigravity => {
            // Its headless mode takes the prompt as `-p`'s argument.
            command
                .args(["-p", prompt, "--output-format", "json", "--json-schema", &schema]);
            if settings.model() != ANTIGRAVITY_DEFAULT_MODEL {
                command.args(["--model", settings.model()]);
            }
            None
        }
        TitleProvider::Unknown(_) => return Err(anyhow!("unknown provider")),
    };
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .with_context(|| format!("running {}", program.display()))?;
    if let (Some(prompt), Some(mut input)) = (stdin, child.stdin.take()) {
        input.write_all(prompt.as_bytes()).await?;
        // Closed, so the CLI knows the prompt is all there.
        drop(input);
    }
    let output = tokio::time::timeout(GENERATION_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| anyhow!("{} timed out", program.display()))??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            stderr
        };
        return Err(anyhow!(
            "{} failed ({}): {detail}",
            program.display(),
            output.status
        ));
    }
    match &settings.provider {
        TitleProvider::Codex => std::fs::read_to_string(&output_path)
            .with_context(|| format!("reading {}", output_path.display())),
        TitleProvider::Claude => claude_title(&output.stdout),
        _ => antigravity_title(&output.stdout),
    }
}

/// Claude's JSON result, or the last result among its messages, as t3code reads it.
fn claude_title(stdout: &[u8]) -> Result<String> {
    let output: Value = serde_json::from_slice(stdout).context("parsing Claude's output")?;
    let envelope = match &output {
        Value::Array(messages) => messages
            .iter()
            .rev()
            .find(|message| message["type"].as_str() == Some("result"))
            .ok_or_else(|| anyhow!("Claude's output has no result"))?,
        envelope => envelope,
    };
    envelope["structured_output"]["title"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Claude returned no title"))
}

/// Antigravity's headless JSON envelope: its structured output, else its response.
fn antigravity_title(stdout: &[u8]) -> Result<String> {
    let envelope: Value =
        serde_json::from_slice(stdout).context("parsing Antigravity's output")?;
    if !envelope["error"].is_null() {
        return Err(anyhow!("Antigravity failed: {}", envelope["error"]));
    }
    envelope["structured_output"]["title"]
        .as_str()
        .or_else(|| envelope["response"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Antigravity returned no title"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_sanitized_as_t3code_does() {
        assert_eq!(
            sanitize_title(r#"{"title": "Fix Login Redirect"}"#).as_deref(),
            Some("Fix Login Redirect")
        );
        assert_eq!(
            sanitize_title("  \"Fix   login\"\nmore").as_deref(),
            Some("Fix login")
        );
        assert_eq!(sanitize_title("``"), None);
        let long = sanitize_title(&"word ".repeat(40)).unwrap_or_default();
        assert_eq!(long.chars().count(), MAX_TITLE_CHARS);
        assert!(long.ends_with("..."));
    }

    #[test]
    fn long_messages_keep_their_start_and_end() {
        let message = format!("start{}end", "x".repeat(10_000));
        let limited = limit_message(&message, MESSAGE_BUDGET);
        assert_eq!(limited.chars().count(), MESSAGE_BUDGET);
        assert!(limited.starts_with("start") && limited.ends_with("end"));
        assert!(limited.contains(TRUNCATED));
        assert_eq!(limit_message("short", MESSAGE_BUDGET), "short");
    }

    #[test]
    fn reads_the_models_codex_lists() {
        let output = json!({"models": [
            {"slug": "gpt-6-luna", "display_name": "GPT-6-Luna", "visibility": "list",
             "supported_reasoning_levels": [{"effort": "low"}, {"effort": "high"}]},
            {"slug": "gpt-reserve", "display_name": "GPT-Reserve", "visibility": "hide"},
        ]});
        let models = parse_codex_models(output.to_string().as_bytes()).unwrap_or_default();
        assert_eq!(
            models,
            [TitleModel {
                id: "gpt-6-luna".into(),
                name: "GPT-6-Luna".into(),
                efforts: vec!["low".into(), "high".into()],
            }]
        );
    }

    #[test]
    fn reads_claudes_result() -> Result<()> {
        let object = json!({"type": "result", "structured_output": {"title": "One"}});
        assert_eq!(claude_title(object.to_string().as_bytes())?, "One");
        let messages = json!([
            {"type": "assistant", "structured_output": {"title": "Not a result"}},
            {"type": "result", "structured_output": {"title": "Two"}},
        ]);
        assert_eq!(claude_title(messages.to_string().as_bytes())?, "Two");
        Ok(())
    }
}
