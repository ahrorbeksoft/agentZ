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
/// t3code's timeout for `codex mcp list`.
const MCP_LIST_TIMEOUT: Duration = Duration::from_secs(10);
/// Plugins and apps bring MCP servers of their own, so t3code turns them off for a title; the
/// user's own servers are listed and turned off by name.
const CODEX_NO_PLUGINS: [&str; 4] = [
    "--config",
    "features.plugins=false",
    "--config",
    "features.apps=false",
];
/// t3code's budget for the user's message in the prompt.
const MESSAGE_BUDGET: usize = 8_000;
const TRUNCATED: &str = "\n[Content truncated]\n";
/// t3code's budget for a thread's messages, when its title is regenerated, and for each.
const CONTEXT_BUDGET: usize = 8_000;
const MESSAGE_LIMIT: isize = 2_000;
const EARLIER_TRUNCATED: &str = "[Earlier content truncated]\n\n";
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

/// t3code's `regenerateThreadTitlePrompt`, about agentZ, up to the previous title.
const REGENERATION_PROMPT_START: &str =
    "Regenerate the title for an existing agentZ thread so the user can recognize it weeks later.
The previous title was ";

/// The rest of t3code's `regenerateThreadTitlePrompt`, after the previous title.
const REGENERATION_PROMPT: &str = "Return JSON with keys title and needsRefinement. Set needsRefinement to false.

Determine the title in this order:
1. Read the USER messages first. Identify the latest explicit durable goal. The original subject remains the subject until the user clearly changes what the thread is about.
2. Use ASSISTANT messages to resolve vague links, unnamed code, and discovered product nouns. Do not promote one assistant finding into the thread subject unless the user adopts it as a new goal.
3. Compare that subject with the previous title. Preserve accurate scope words, especially when earlier content is truncated. Replace the previous title when it is generic, artifact-based, a completion update, or contradicted by the thread.
4. Title the durable subject and desired outcome, not the current workflow state.

Editorial rules:
- 3-8 words, fewer than 40 characters.
- Use a compact noun phrase or clear action phrase.
- Preserve the umbrella subject when later messages focus on one finding, provider, platform, or implementation detail.
- A thread progressing through research, planning, implementation, review, CI, merge, and monitoring has usually not changed subjects.
- Ignore deliverables and operations such as mocks, plans, HTML, branches, PRs, tests, CI, commits, merging, and monitoring unless they are the actual topic.
- Models, subagents, tools, output formats, and monitoring instructions do not belong in the title unless they are themselves the topic.
- Treat final operational follow-ups and assistant completion summaries as weak evidence of subject.
- For reviews, name the reviewed feature or system and its durable concern, not one finding from the review.
- For research, name the question domain rather than the research process.
- Do not claim the work is complete.
- Do not copy and truncate a thread message.
- Avoid project names already visible in the UI, PR numbers, quotes, labels, filler, and trailing punctuation.
- Use attached images as primary context for UI issues.
- When a URL or attachment is the only source of the subject, use available tools to inspect it directly.
- Local git history is not evidence of what a linked PR or issue is about. Never title the thread after branch names, commit messages, or merged commits found in the checkout.
- If a linked PR or issue cannot be read, fall back to the user's stated action plus its number, such as \"Take Over PR 8588\". This is the one case where a PR or issue number belongs in the title.
- Keep the previous title unchanged if it is already accurate. Otherwise return a meaningfully improved title, not a cosmetic paraphrase.

Examples of the distinction:
- A subagent-monitoring review that finds a Codex roster bug remains \"Review Subagent Monitoring Risks,\" not \"Codex Roster Bug Review.\"
- A vague failing-test request later identified as a lazy thread-feed mismatch becomes \"Fix Lazy Thread Feed Test,\" not \"Prevent Mobile Feed Regressions.\"
- A QR-sharing overhaul that ends with CI and merge work remains about QR sharing, not the PR lifecycle.";

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

/// The MCP servers the user's Codex would start, by name, as t3code lists them. None if Codex
/// can't list them: the title is still written, with them.
async fn codex_mcp_servers(
    program: &Path,
    directory: &Path,
    search_path: Option<&OsString>,
) -> Vec<String> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(CODEX_NO_PLUGINS)
        .args(["mcp", "list", "--json"])
        .current_dir(directory)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    if let Some(search_path) = search_path {
        command.env("PATH", search_path);
    }
    let failure = match tokio::time::timeout(MCP_LIST_TIMEOUT, command.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            match parse_codex_mcp_servers(&output.stdout) {
                Some(servers) => return servers,
                None => "its list didn't parse".to_string(),
            }
        }
        Ok(Ok(output)) => format!(
            "{}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Ok(Err(error)) => error.to_string(),
        Err(_) => "it timed out".to_string(),
    };
    log::warn!("codex mcp list failed, so titles keep its MCP servers: {failure}");
    Vec::new()
}

fn parse_codex_mcp_servers(output: &[u8]) -> Option<Vec<String>> {
    #[derive(serde::Deserialize)]
    struct Server {
        name: String,
        enabled: bool,
    }
    let servers: Vec<Server> = serde_json::from_slice(output).ok()?;
    Some(
        servers
            .into_iter()
            .filter(|server| server.enabled)
            .map(|server| server.name)
            .collect(),
    )
}

/// The `--config` value that turns off each of these MCP servers. `mcp_servers={}` would merge
/// into the user's servers rather than replace them, so each is named, quoted so that a name
/// with a dot stays one key.
fn codex_mcp_servers_off(names: &[String]) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    let servers = names
        .iter()
        .map(|name| format!("{}={{enabled=false}}", Value::from(name.as_str())))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("mcp_servers={{{servers}}}"))
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

/// t3code's `buildThreadTitlePrompt` with a previous title: its regeneration prompt, about
/// agentZ, over the thread's messages ([`thread_context`]).
pub(crate) fn regeneration_prompt(previous_title: &str, contents: &str) -> String {
    let previous_title = serde_json::to_string(previous_title).unwrap_or_default();
    format!(
        "{REGENERATION_PROMPT_START}{previous_title}.\n{REGENERATION_PROMPT}\n\nThread contents:\n{contents}"
    )
}

/// Who said a message, for [`thread_context`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Speaker {
    User,
    Agent,
}

impl Speaker {
    fn prefix(self) -> &'static str {
        match self {
            Speaker::User => "USER:\n",
            Speaker::Agent => "ASSISTANT:\n",
        }
    }
}

/// t3code's `formatThreadTitleContext`: the thread's messages in order, each under who said
/// it, within a budget. The first user message comes first, then the latest user messages, up
/// to 6,000 characters the agent's replies can't take, then the agent's latest replies, and
/// any room left lets the chosen ones grow.
pub(crate) fn thread_context(messages: &[(Speaker, String)]) -> String {
    let sections: Vec<(Speaker, &str)> = messages
        .iter()
        .map(|(speaker, text)| (*speaker, text.trim()))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let mut context = Context {
        sections: &sections,
        selected: vec![None; sections.len()],
        remaining: (CONTEXT_BUDGET - EARLIER_TRUNCATED.len()) as isize,
    };
    let from_latest = |speaker: Speaker| -> Vec<usize> {
        (0..sections.len())
            .rev()
            .filter(|index| sections[*index].0 == speaker)
            .collect()
    };
    if let Some(first) = sections
        .iter()
        .position(|(speaker, _)| *speaker == Speaker::User)
    {
        context.add(first, MESSAGE_LIMIT);
    }
    for index in from_latest(Speaker::User) {
        context.add(index, MESSAGE_LIMIT.min(context.remaining - 2_000));
    }
    for index in from_latest(Speaker::Agent) {
        context.add(index, MESSAGE_LIMIT);
    }
    // A short thread's messages fill the room left.
    for speaker in [Speaker::User, Speaker::Agent] {
        for index in from_latest(speaker) {
            context.expand(index);
        }
    }
    let shortened = sections
        .iter()
        .zip(&context.selected)
        .any(|((speaker, text), section)| match section {
            Some(section) => *section != format!("{}{text}", speaker.prefix()),
            None => true,
        });
    let retained: Vec<String> = context.selected.into_iter().flatten().collect();
    let contents = retained.join("\n\n");
    if shortened && !contents.is_empty() {
        format!("{EARLIER_TRUNCATED}{contents}")
    } else {
        contents
    }
}

/// The messages [`thread_context`] has chosen so far, and the room left.
struct Context<'a> {
    sections: &'a [(Speaker, &'a str)],
    selected: Vec<Option<String>>,
    remaining: isize,
}

impl Context<'_> {
    /// Chooses the message, shortened to `budget` or the room left, unless it's chosen already
    /// or too little room is left for it.
    fn add(&mut self, index: usize, budget: isize) {
        if self.selected[index].is_some() {
            return;
        }
        let (speaker, text) = self.sections[index];
        let limit = budget.min(self.remaining) - chars(speaker.prefix()) - 2;
        if limit <= chars(TRUNCATED) {
            return;
        }
        let section = format!(
            "{}{}",
            speaker.prefix(),
            limit_message(text, limit as usize)
        );
        self.remaining -= chars(&section) + 2;
        self.selected[index] = Some(section);
    }

    /// Lets a chosen message grow into the room left.
    fn expand(&mut self, index: usize) {
        let Some(previous) = self.selected[index].as_ref() else {
            return;
        };
        let (speaker, text) = self.sections[index];
        let budget = chars(previous) + self.remaining - chars(speaker.prefix());
        let expanded = format!(
            "{}{}",
            speaker.prefix(),
            limit_message(text, budget.max(0) as usize)
        );
        self.remaining -= chars(&expanded) - chars(previous);
        self.selected[index] = Some(expanded);
    }
}

fn chars(text: &str) -> isize {
    text.chars().count() as isize
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

/// A title from the chosen CLI on `search_path` (the server's `PATH` if `None`), for the
/// [`prompt`] of a thread's first message or the [`regeneration_prompt`] of its messages.
pub(crate) async fn generate(
    settings: TitleGeneration,
    prompt: String,
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
    let result = run(&settings, &program, &directory, &prompt, search_path).await;
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
    let mcp_servers_off = match &settings.provider {
        TitleProvider::Codex => codex_mcp_servers_off(
            &codex_mcp_servers(program, directory, search_path.as_ref()).await,
        ),
        _ => None,
    };
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
            // A title needs only its prompt. The user's MCP servers would otherwise start
            // and could act on it, as Claude's `--strict-mcp-config` prevents for Claude.
            command.arg("exec").args(CODEX_NO_PLUGINS);
            if let Some(mcp_servers_off) = &mcp_servers_off {
                command.arg("--config").arg(mcp_servers_off);
            }
            command
                .args(["--ephemeral", "--skip-git-repo-check", "-s", "read-only"])
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
            command.args([
                "-p",
                prompt,
                "--output-format",
                "json",
                "--json-schema",
                &schema,
            ]);
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
    let envelope: Value = serde_json::from_slice(stdout).context("parsing Antigravity's output")?;
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

    /// A short thread's messages go in whole, in order; a long one's keep the first and latest
    /// user messages over the agent's replies, under a note that some were left out.
    #[test]
    fn a_threads_messages_fit_t3codes_budget() {
        let short = [
            (Speaker::User, "fix the login redirect".to_string()),
            (Speaker::Agent, "Fixed it in auth.ts.".to_string()),
            (Speaker::User, "  ".to_string()),
        ];
        assert_eq!(
            thread_context(&short),
            "USER:\nfix the login redirect\n\nASSISTANT:\nFixed it in auth.ts."
        );
        assert_eq!(thread_context(&[]), "");

        let long: Vec<(Speaker, String)> = (0..20)
            .flat_map(|turn| {
                [
                    (Speaker::User, format!("ask {turn} {}", "u".repeat(900))),
                    (Speaker::Agent, format!("reply {turn} {}", "a".repeat(900))),
                ]
            })
            .collect();
        let context = thread_context(&long);
        assert!(context.starts_with(EARLIER_TRUNCATED));
        assert!(context.chars().count() <= CONTEXT_BUDGET);
        assert!(context.contains("USER:\nask 0 "));
        assert!(context.contains("USER:\nask 19 "));
        assert!(context.contains("ASSISTANT:\nreply 19 "));
        assert!(!context.contains("ASSISTANT:\nreply 0 "));
        let first = context.find("ask 0 ").expect("the first message");
        let latest = context.find("ask 19 ").expect("the latest message");
        assert!(first < latest, "in the thread's order");
    }

    #[test]
    fn regeneration_names_the_previous_title() {
        let prompt = regeneration_prompt("Login \"redirect\"", "USER:\nfix it");
        assert!(prompt.starts_with(
            "Regenerate the title for an existing agentZ thread so the user can recognize it \
             weeks later.\nThe previous title was \"Login \\\"redirect\\\"\".\nReturn JSON"
        ));
        assert!(prompt.ends_with("\n\nThread contents:\nUSER:\nfix it"));
    }

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
    fn turns_off_the_mcp_servers_codex_starts() {
        let output = json!([
            {"name": "augmenter", "enabled": true},
            {"name": "my.server", "enabled": true},
            {"name": "already-off", "enabled": false},
        ]);
        let servers = parse_codex_mcp_servers(output.to_string().as_bytes()).unwrap_or_default();
        assert_eq!(servers, ["augmenter", "my.server"]);
        assert_eq!(
            codex_mcp_servers_off(&servers).as_deref(),
            Some(r#"mcp_servers={"augmenter"={enabled=false},"my.server"={enabled=false}}"#)
        );
        assert_eq!(codex_mcp_servers_off(&[]), None);
        assert_eq!(parse_codex_mcp_servers(br#"[{"name": "x"}]"#), None);
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
