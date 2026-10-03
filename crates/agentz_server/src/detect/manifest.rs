//! herdr's detection manifests (`src/detect/manifest.rs`): per-agent rules that read a
//! terminal's screen, and its title and progress, and name the agent's state. The bundled
//! manifests are herdr's, unchanged; herdr's remote updates and local overrides are left out.
//!
//! Ported from herdr (https://github.com/herdrdev/herdr), licensed under the Apache License,
//! Version 2.0 (see `LICENSE-APACHE`). Changed: manifests are only bundled, and the explain
//! output is gone.

use std::sync::{Arc, OnceLock};

use collections::HashMap;
use regex::Regex;
use serde::Deserialize;

use super::{Agent, AgentDetection, AgentState};

/// What the rules read: the bottom of the screen, and the strings programs set through OSC
/// sequences. Empty strings stand for what the terminal didn't capture.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DetectionInput<'a> {
    pub screen: &'a str,
    pub osc_title: &'a str,
    pub osc_progress: &'a str,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentManifest {
    id: String,
    #[serde(rename = "version")]
    _version: Option<String>,
    min_engine_version: Option<u32>,
    #[serde(rename = "updated_at")]
    _updated_at: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    rules: Vec<ManifestRule>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct ManifestRule {
    id: String,
    state: Option<ManifestState>,
    #[serde(default)]
    priority: i32,
    #[serde(default = "default_region")]
    region: String,
    #[serde(default)]
    visible_idle: bool,
    #[serde(default)]
    visible_blocker: bool,
    #[serde(default)]
    visible_working: bool,
    #[serde(default)]
    skip_state_update: bool,
    #[serde(default)]
    all: Vec<ManifestGate>,
    #[serde(default)]
    any: Vec<ManifestGate>,
    #[serde(default, rename = "not")]
    not_gate: Vec<ManifestGate>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct ManifestGate {
    #[serde(default)]
    all: Vec<ManifestGate>,
    #[serde(default)]
    any: Vec<ManifestGate>,
    #[serde(default, rename = "not")]
    not_gate: Vec<ManifestGate>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledGate {
    all: Vec<CompiledGate>,
    any: Vec<CompiledGate>,
    not_gate: Vec<CompiledGate>,
    contains: Vec<String>,
    regex: Vec<Regex>,
    line_regex: Vec<Regex>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ManifestState {
    Idle,
    Working,
    Blocked,
    Unknown,
}

impl From<ManifestState> for AgentState {
    fn from(value: ManifestState) -> Self {
        match value {
            ManifestState::Idle => AgentState::Idle,
            ManifestState::Working => AgentState::Working,
            ManifestState::Blocked => AgentState::Blocked,
            ManifestState::Unknown => AgentState::Unknown,
        }
    }
}

fn default_region() -> String {
    "whole_recent".to_string()
}

/// A manifest with its rules compiled, kept for the server's lifetime so the regexes' caches
/// stay warm across polls (herdr).
struct LoadedManifest {
    manifest: AgentManifest,
    compiled_rules: Arc<[CompiledGate]>,
}

const BUNDLED_MANIFESTS: &[(&str, &str)] = &[
    ("amp", include_str!("manifests/amp.toml")),
    ("agy", include_str!("manifests/antigravity.toml")),
    ("claude", include_str!("manifests/claude.toml")),
    ("cline", include_str!("manifests/cline.toml")),
    ("codex", include_str!("manifests/codex.toml")),
    ("cursor", include_str!("manifests/cursor.toml")),
    ("devin", include_str!("manifests/devin.toml")),
    ("droid", include_str!("manifests/droid.toml")),
    ("gemini", include_str!("manifests/gemini.toml")),
    ("grok", include_str!("manifests/grok.toml")),
    ("hermes", include_str!("manifests/hermes.toml")),
    ("kilo", include_str!("manifests/kilo.toml")),
    ("kimi", include_str!("manifests/kimi.toml")),
    ("kiro", include_str!("manifests/kiro.toml")),
    ("letta", include_str!("manifests/letta.toml")),
    ("maki", include_str!("manifests/maki.toml")),
    ("muse", include_str!("manifests/muse.toml")),
    ("opencode", include_str!("manifests/opencode.toml")),
    ("pi", include_str!("manifests/pi.toml")),
    ("qodercli", include_str!("manifests/qodercli.toml")),
    ("qwen", include_str!("manifests/qwen.toml")),
    ("copilot", include_str!("manifests/github-copilot.toml")),
];

const MAX_RULES_PER_MANIFEST: usize = 128;
const MAX_GATE_DEPTH: usize = 8;
const MAX_TOTAL_GATES: usize = 512;
const MAX_MATCHERS_PER_GATE: usize = 32;
const MAX_TOTAL_MATCHERS: usize = 1024;
const MAX_MATCHER_CHARS: usize = 512;
const TOP_NON_EMPTY_LINES_ENGINE_VERSION: u32 = 3;
const MAX_TOP_REGION_LINE_COUNT: usize = u16::MAX as usize;

fn manifests() -> &'static HashMap<Agent, LoadedManifest> {
    static MANIFESTS: OnceLock<HashMap<Agent, LoadedManifest>> = OnceLock::new();
    MANIFESTS.get_or_init(|| {
        Agent::SCREEN_MANIFEST_AGENTS
            .into_iter()
            .filter_map(|agent| {
                let manifest = bundled_manifest(agent)?;
                if !manifest_matches_agent(&manifest, agent) {
                    log::error!(
                        "the bundled {} manifest is for {}",
                        agent.label(),
                        manifest.id
                    );
                    return None;
                }
                match compile_manifest(&manifest) {
                    Ok(compiled_rules) => Some((
                        agent,
                        LoadedManifest {
                            manifest,
                            compiled_rules: compiled_rules.into(),
                        },
                    )),
                    Err(error) => {
                        log::error!(
                            "the bundled {} manifest doesn't compile: {error}",
                            agent.label()
                        );
                        None
                    }
                }
            })
            .collect()
    })
}

fn bundled_manifest(agent: Agent) -> Option<AgentManifest> {
    let id = agent.label();
    let (_, content) = BUNDLED_MANIFESTS
        .iter()
        .find(|(manifest_id, _)| *manifest_id == id)?;
    parse_manifest(content)
        .map_err(|error| log::error!("the bundled {id} manifest is invalid: {error}"))
        .ok()
}

/// The agent's state on this screen. The highest-priority matching rule wins; a known agent
/// that matches nothing is idle (herdr's `default_known_agent_idle_fallback`).
pub(crate) fn detect(agent: Agent, input: DetectionInput<'_>) -> AgentDetection {
    let Some(loaded) = manifests().get(&agent) else {
        return AgentDetection::idle();
    };
    evaluate(&loaded.manifest, &loaded.compiled_rules, input)
}

fn evaluate(
    manifest: &AgentManifest,
    compiled_rules: &[CompiledGate],
    input: DetectionInput<'_>,
) -> AgentDetection {
    let mut matched: Option<&ManifestRule> = None;
    for (rule, compiled) in manifest.rules.iter().zip(compiled_rules) {
        if !compiled_rule_matches(compiled, region(input, &rule.region)) {
            continue;
        }
        match matched {
            Some(previous) if previous.priority >= rule.priority => {}
            _ => matched = Some(rule),
        }
    }
    let Some(rule) = matched else {
        return AgentDetection::idle();
    };
    let state = rule
        .state
        .map(AgentState::from)
        .unwrap_or(AgentState::Unknown);
    AgentDetection {
        state,
        skip_state_update: rule.skip_state_update,
        visible_idle: rule.visible_idle && state == AgentState::Idle,
        visible_blocker: rule.visible_blocker && state == AgentState::Blocked,
        visible_working: rule.visible_working && state == AgentState::Working,
    }
}

pub(crate) fn parse_manifest(content: &str) -> Result<AgentManifest, String> {
    let manifest = toml::from_str::<AgentManifest>(content).map_err(|err| err.to_string())?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &AgentManifest) -> Result<(), String> {
    if manifest.rules.is_empty() {
        return Err("manifest must contain at least one rule".to_string());
    }
    if manifest.rules.len() > MAX_RULES_PER_MANIFEST {
        return Err(format!(
            "manifest contains {} rules, max is {MAX_RULES_PER_MANIFEST}",
            manifest.rules.len()
        ));
    }

    let mut complexity = ManifestComplexity::default();
    for rule in &manifest.rules {
        if rule.id.trim().is_empty() {
            return Err("manifest rule id must not be empty".to_string());
        }
        if rule.skip_state_update {
            if rule.state != Some(ManifestState::Unknown) {
                return Err(format!(
                    "rule {} uses skip_state_update without state = \"unknown\"",
                    rule.id
                ));
            }
            if rule.visible_idle || rule.visible_blocker || rule.visible_working {
                return Err(format!(
                    "rule {} uses skip_state_update with visible state evidence",
                    rule.id
                ));
            }
        }
        validate_region_name(&rule.region)
            .map_err(|err| format!("rule {} uses invalid region: {err}", rule.id))?;
        if rule.region.trim().starts_with("top_non_empty_lines(")
            && manifest
                .min_engine_version
                .is_some_and(|version| version < TOP_NON_EMPTY_LINES_ENGINE_VERSION)
        {
            return Err(format!(
                "rule {} uses top_non_empty_lines but min_engine_version is below {}",
                rule.id, TOP_NON_EMPTY_LINES_ENGINE_VERSION
            ));
        }
        validate_gate(&manifest_gate_from_rule(rule), "rule", 0, &mut complexity)
            .map_err(|err| format!("rule {} has invalid matcher gates: {err}", rule.id))?;
    }

    Ok(())
}

#[derive(Default)]
struct ManifestComplexity {
    total_gates: usize,
    total_matchers: usize,
}

fn validate_gate(
    gate: &ManifestGate,
    context: &str,
    depth: usize,
    complexity: &mut ManifestComplexity,
) -> Result<(), String> {
    if depth > MAX_GATE_DEPTH {
        return Err(format!("{context} exceeds max gate depth {MAX_GATE_DEPTH}"));
    }
    complexity.total_gates += 1;
    if complexity.total_gates > MAX_TOTAL_GATES {
        return Err(format!("manifest exceeds max gate count {MAX_TOTAL_GATES}"));
    }
    validate_matcher_limits(gate, context, complexity)?;
    if !gate_has_positive_matcher(gate) {
        return Err(format!("{context} must contain a positive matcher"));
    }
    validate_regex_patterns(&gate.regex, context, "regex")?;
    validate_regex_patterns(&gate.line_regex, context, "line_regex")?;
    for nested in &gate.all {
        validate_gate(nested, "all gate", depth + 1, complexity)?;
    }
    for nested in &gate.any {
        validate_gate(nested, "any gate", depth + 1, complexity)?;
    }
    for nested in &gate.not_gate {
        if !gate_has_any_matcher(nested) {
            return Err(format!("{context} contains an empty not gate"));
        }
        validate_not_gate(nested, depth + 1, complexity)?;
    }
    Ok(())
}

fn validate_not_gate(
    gate: &ManifestGate,
    depth: usize,
    complexity: &mut ManifestComplexity,
) -> Result<(), String> {
    if depth > MAX_GATE_DEPTH {
        return Err(format!("not gate exceeds max gate depth {MAX_GATE_DEPTH}"));
    }
    complexity.total_gates += 1;
    if complexity.total_gates > MAX_TOTAL_GATES {
        return Err(format!("manifest exceeds max gate count {MAX_TOTAL_GATES}"));
    }
    validate_matcher_limits(gate, "not gate", complexity)?;
    if !gate_has_any_matcher(gate) {
        return Err("not gate must contain a matcher".to_string());
    }
    validate_regex_patterns(&gate.regex, "not gate", "regex")?;
    validate_regex_patterns(&gate.line_regex, "not gate", "line_regex")?;
    for nested in &gate.all {
        validate_gate(nested, "not all gate", depth + 1, complexity)?;
    }
    for nested in &gate.any {
        validate_gate(nested, "not any gate", depth + 1, complexity)?;
    }
    for nested in &gate.not_gate {
        validate_not_gate(nested, depth + 1, complexity)?;
    }
    Ok(())
}

fn validate_matcher_limits(
    gate: &ManifestGate,
    context: &str,
    complexity: &mut ManifestComplexity,
) -> Result<(), String> {
    let matcher_count = gate.contains.len() + gate.regex.len() + gate.line_regex.len();
    if matcher_count > MAX_MATCHERS_PER_GATE {
        return Err(format!(
            "{context} has {matcher_count} direct matchers, max is {MAX_MATCHERS_PER_GATE}"
        ));
    }
    complexity.total_matchers += matcher_count;
    if complexity.total_matchers > MAX_TOTAL_MATCHERS {
        return Err(format!(
            "manifest exceeds max matcher count {MAX_TOTAL_MATCHERS}"
        ));
    }
    for value in gate
        .contains
        .iter()
        .chain(gate.regex.iter())
        .chain(gate.line_regex.iter())
    {
        if value.chars().count() > MAX_MATCHER_CHARS {
            return Err(format!(
                "{context} matcher exceeds max length {MAX_MATCHER_CHARS}"
            ));
        }
    }
    Ok(())
}

fn validate_regex_patterns(patterns: &[String], context: &str, field: &str) -> Result<(), String> {
    for pattern in patterns {
        Regex::new(pattern).map_err(|err| {
            format!("{context} contains invalid {field} pattern {pattern:?}: {err}")
        })?;
    }
    Ok(())
}

fn gate_has_positive_matcher(gate: &ManifestGate) -> bool {
    !gate.contains.is_empty()
        || !gate.regex.is_empty()
        || !gate.line_regex.is_empty()
        || !gate.all.is_empty()
        || !gate.any.is_empty()
}

fn gate_has_any_matcher(gate: &ManifestGate) -> bool {
    gate_has_positive_matcher(gate) || !gate.not_gate.is_empty()
}

fn validate_region_name(spec: &str) -> Result<(), String> {
    let trimmed = spec.trim();
    match trimmed {
        "whole_recent"
        | "after_last_prompt_marker"
        | "before_current_prompt_marker"
        | "whole_recent_without_current_prompt_marker"
        | "current_prompt_block_marker"
        | "after_current_prompt_block_marker"
        | "prompt_box_body"
        | "above_prompt_box"
        | "last_non_empty_above_prompt_box"
        | "after_last_horizontal_rule"
        | "osc_title"
        | "osc_progress" => Ok(()),
        _ if region_count(trimmed, "bottom_lines").is_some()
            || region_count(trimmed, "bottom_non_empty_lines").is_some()
            || top_region_count(trimmed).is_some() =>
        {
            Ok(())
        }
        _ => Err(trimmed.to_string()),
    }
}

/// Whether a manifest is for `agent`, by its id or an alias.
fn manifest_matches_agent(manifest: &AgentManifest, agent: Agent) -> bool {
    let id = agent.label();
    manifest.id == id
        || manifest.aliases.iter().any(|alias| alias == id)
        || Agent::from_label(&manifest.id) == Some(agent)
        || manifest
            .aliases
            .iter()
            .any(|alias| Agent::from_label(alias) == Some(agent))
}

fn manifest_gate_from_rule(rule: &ManifestRule) -> ManifestGate {
    ManifestGate {
        all: rule.all.clone(),
        any: rule.any.clone(),
        not_gate: rule.not_gate.clone(),
        contains: rule.contains.clone(),
        regex: rule.regex.clone(),
        line_regex: rule.line_regex.clone(),
    }
}

fn compile_manifest(manifest: &AgentManifest) -> Result<Vec<CompiledGate>, String> {
    manifest
        .rules
        .iter()
        .map(|rule| {
            compile_gate(&manifest_gate_from_rule(rule))
                .map_err(|err| format!("rule {} could not be compiled: {err}", rule.id))
        })
        .collect()
}

fn compile_gate(gate: &ManifestGate) -> Result<CompiledGate, String> {
    Ok(CompiledGate {
        all: gate
            .all
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        any: gate
            .any
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        not_gate: gate
            .not_gate
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        contains: gate
            .contains
            .iter()
            .map(|needle| needle.to_lowercase())
            .collect(),
        regex: gate
            .regex
            .iter()
            .map(|pattern| Regex::new(pattern).map_err(|err| err.to_string()))
            .collect::<Result<_, _>>()?,
        line_regex: gate
            .line_regex
            .iter()
            .map(|pattern| Regex::new(pattern).map_err(|err| err.to_string()))
            .collect::<Result<_, _>>()?,
    })
}

fn compiled_rule_matches(gate: &CompiledGate, text: &str) -> bool {
    let lower_text = text.to_lowercase();
    compiled_gate_matches(gate, text, &lower_text)
}

fn compiled_gate_matches(gate: &CompiledGate, text: &str, lower_text: &str) -> bool {
    if !gate
        .contains
        .iter()
        .all(|needle| lower_text.contains(needle))
    {
        return false;
    }

    if !gate.regex.iter().all(|regex| regex.is_match(text)) {
        return false;
    }

    if !gate
        .line_regex
        .iter()
        .all(|regex| text.lines().any(|line| regex.is_match(line)))
    {
        return false;
    }

    if !gate
        .all
        .iter()
        .all(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    if !gate.any.is_empty()
        && !gate
            .any
            .iter()
            .any(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    if gate
        .not_gate
        .iter()
        .any(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    true
}

fn region<'a>(input: DetectionInput<'a>, spec: &str) -> &'a str {
    let trimmed = spec.trim();
    match trimmed {
        "osc_title" => return input.osc_title,
        "osc_progress" => return input.osc_progress,
        _ => {}
    }
    let content = input.screen;
    match trimmed {
        "whole_recent" => content,
        "after_last_prompt_marker" => after_last_prompt_marker(content),
        "before_current_prompt_marker" => before_current_prompt_marker(content),
        "whole_recent_without_current_prompt_marker" => {
            whole_recent_without_current_prompt_marker(content)
        }
        "current_prompt_block_marker" => current_prompt_block_marker(content).unwrap_or(""),
        "after_current_prompt_block_marker" => {
            after_current_prompt_block_marker(content).unwrap_or("")
        }
        "prompt_box_body" => prompt_box_body(content).unwrap_or(""),
        "above_prompt_box" => above_prompt_box(content),
        "last_non_empty_above_prompt_box" => last_non_empty_line(above_prompt_box(content)),
        "after_last_horizontal_rule" => after_last_horizontal_rule(content),
        _ => {
            if let Some(count) = region_count(trimmed, "bottom_lines") {
                return bottom_lines(content, count);
            }
            if let Some(count) = region_count(trimmed, "bottom_non_empty_lines") {
                return bottom_non_empty_lines(content, count);
            }
            if let Some(count) = top_region_count(trimmed) {
                return top_non_empty_lines(content, count);
            }
            ""
        }
    }
}

fn region_count(spec: &str, name: &str) -> Option<usize> {
    spec.strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|count| count.parse::<usize>().ok())
}

fn top_region_count(spec: &str) -> Option<usize> {
    let count = spec
        .strip_prefix("top_non_empty_lines")?
        .strip_prefix('(')?
        .strip_suffix(')')?;
    if count.starts_with('0') || !count.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    count
        .parse::<usize>()
        .ok()
        .filter(|count| *count <= MAX_TOP_REGION_LINE_COUNT)
}

fn bottom_lines(content: &str, count: usize) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(count);
    slice_from_line_index(content, &lines, start)
}

fn bottom_non_empty_lines(content: &str, count: usize) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let Some(start_index) = lines
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, line)| !line.trim().is_empty())
        .take(count)
        .last()
        .map(|(index, _)| index)
    else {
        return "";
    };
    slice_from_line_index(content, &lines, start_index)
}

fn top_non_empty_lines(content: &str, count: usize) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let Some(end_index) = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .take(count)
        .last()
        .map(|(index, _)| index)
    else {
        return "";
    };
    let byte_offset = line_start_offset(content, &lines, end_index + 1);
    &content[..byte_offset]
}

fn after_last_prompt_marker(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let Some(index) = lines.iter().rposition(|line| codex_prompt_line(line)) else {
        return content;
    };
    slice_from_line_index(content, &lines, index + 1)
}

fn before_current_prompt_marker(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let Some(index) = current_codex_prompt_index(&lines) else {
        return content;
    };
    let byte_offset = lines[..index]
        .iter()
        .map(|line| line.len() + 1)
        .sum::<usize>();
    &content[..byte_offset.min(content.len())]
}

fn whole_recent_without_current_prompt_marker(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    if current_codex_prompt_index(&lines).is_some() {
        ""
    } else {
        content
    }
}

fn current_prompt_block_marker(content: &str) -> Option<&str> {
    let lines: Vec<&str> = content.lines().collect();
    let prompt_index = current_codex_prompt_index(&lines)?;
    lines[..prompt_index]
        .iter()
        .rev()
        .find(|line| codex_block_marker_line(line))
        .copied()
}

fn after_current_prompt_block_marker(content: &str) -> Option<&str> {
    let lines: Vec<&str> = content.lines().collect();
    let prompt_index = current_codex_prompt_index(&lines)?;
    let block_index = lines[..prompt_index]
        .iter()
        .rposition(|line| codex_block_marker_line(line))?;
    Some(slice_from_line_index(content, &lines, block_index))
}

fn current_codex_prompt_index(lines: &[&str]) -> Option<usize> {
    let prompt_index = lines.iter().rposition(|line| codex_prompt_line(line))?;
    if lines[prompt_index + 1..]
        .iter()
        .any(|line| codex_block_marker_line(line))
    {
        return None;
    }
    Some(prompt_index)
}

fn codex_prompt_line(line: &str) -> bool {
    line == "›" || line.starts_with("› ")
}

fn codex_block_marker_line(line: &str) -> bool {
    line.starts_with('•') || line.starts_with('■') || line.starts_with('✗') || line.starts_with('✓')
}

fn prompt_box_body(content: &str) -> Option<&str> {
    let lines: Vec<&str> = content.lines().collect();
    let top = prompt_box_top_border_index(&lines)?;
    let start = line_start_offset(content, &lines, top + 1);
    let end_index = lines[top + 1..]
        .iter()
        .position(|line| is_horizontal_rule(line))
        .map(|relative| top + 1 + relative)
        .unwrap_or(lines.len());
    let end = line_start_offset(content, &lines, end_index);
    Some(&content[start.min(content.len())..end.min(content.len())])
}

fn above_prompt_box(content: &str) -> &str {
    let lines: Vec<&str> = content.lines().collect();
    let Some(top) = prompt_box_top_border_index(&lines) else {
        return content;
    };
    let end = line_start_offset(content, &lines, top);
    &content[..end.min(content.len())]
}

fn after_last_horizontal_rule(content: &str) -> &str {
    let mut last_rule_end = 0usize;
    let mut offset = 0usize;
    for line in content.lines() {
        let next_offset = offset + line.len() + 1;
        if is_horizontal_rule(line) {
            last_rule_end = next_offset.min(content.len());
        }
        offset = next_offset;
    }
    &content[last_rule_end..]
}

fn last_non_empty_line(content: &str) -> &str {
    content
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
}

fn prompt_box_top_border_index(lines: &[&str]) -> Option<usize> {
    let mut border_count = 0;
    for index in (0..lines.len()).rev() {
        if is_horizontal_rule(lines[index]) {
            border_count += 1;
            if border_count == 2 {
                return Some(index);
            }
        }
    }
    None
}

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let rule_chars = trimmed.chars().take_while(|&ch| ch == '─').count();
    if rule_chars == 0 {
        return false;
    }

    let rule_bytes = trimmed
        .char_indices()
        .nth(rule_chars)
        .map(|(index, _)| index)
        .unwrap_or(trimmed.len());
    let suffix = trimmed[rule_bytes..].trim_start();

    suffix.is_empty() || rule_chars >= 3
}

fn slice_from_line_index<'a>(content: &'a str, lines: &[&str], index: usize) -> &'a str {
    let byte_offset = line_start_offset(content, lines, index);
    &content[byte_offset.min(content.len())..]
}

fn line_start_offset(content: &str, lines: &[&str], index: usize) -> usize {
    lines[..index.min(lines.len())]
        .iter()
        .map(|line| line.len() + 1)
        .sum::<usize>()
        .min(content.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(screen: &str) -> DetectionInput<'_> {
        DetectionInput {
            screen,
            osc_title: "",
            osc_progress: "",
        }
    }

    fn rules_manifest(rules: &str) -> String {
        format!("id = \"codex\"\n\n{rules}\n")
    }

    /// Evaluates a manifest that isn't bundled, as herdr's tests do with overrides.
    fn detect_with(manifest: &str, input: DetectionInput<'_>) -> AgentDetection {
        let manifest = parse_manifest(manifest).expect("a valid manifest");
        let compiled = compile_manifest(&manifest).expect("compiled rules");
        evaluate(&manifest, &compiled, input)
    }

    #[test]
    fn all_bundled_manifests_parse_and_match_their_agent() {
        for agent in Agent::SCREEN_MANIFEST_AGENTS {
            let manifest = bundled_manifest(agent)
                .unwrap_or_else(|| panic!("missing bundled manifest for {}", agent.label()));
            assert!(
                manifest_matches_agent(&manifest, agent),
                "{}",
                agent.label()
            );
            assert!(manifests().contains_key(&agent), "{}", agent.label());
        }
    }

    #[test]
    fn known_agent_no_match_defaults_to_idle() {
        let detection = detect(Agent::Codex, input("unmatched-marker"));
        assert_eq!(detection.state, AgentState::Idle);
        assert!(!detection.visible_idle);
    }

    #[test]
    fn rule_semantics_apply_gates_priority_and_line_regex() {
        let manifest = rules_manifest(
            r#"
[[rules]]
id = "low_contains"
state = "idle"
priority = 1
contains = ["match"]

[[rules]]
id = "high_nested_gates"
state = "working"
priority = 10
contains = ["match"]
all = [
  { any = [{ regex = ["w[io]n"] }, { contains = ["fallback"] }] },
]
not = [
  { contains = ["blocked"] },
]

[[rules]]
id = "line_regex"
state = "blocked"
priority = 20
line_regex = ["^exact line$"]
"#,
        );
        assert_eq!(
            detect_with(&manifest, input("match win")).state,
            AgentState::Working
        );
        assert_eq!(
            detect_with(&manifest, input("MATCH fallback")).state,
            AgentState::Working
        );
        assert_eq!(
            detect_with(&manifest, input("match win blocked")).state,
            AgentState::Idle
        );
        assert_eq!(
            detect_with(&manifest, input("match\nexact line\n")).state,
            AgentState::Blocked
        );
        assert_eq!(
            detect_with(&manifest, input("not an exact line")).state,
            AgentState::Idle
        );
    }

    #[test]
    fn osc_regions_read_their_own_inputs() {
        let manifest = rules_manifest(
            r#"
[[rules]]
id = "title"
state = "working"
priority = 10
region = "osc_title"
contains = ["busy"]

[[rules]]
id = "progress"
state = "idle"
priority = 5
region = "osc_progress"
regex = ["^4;0"]
"#,
        );
        let screen_only = DetectionInput {
            screen: "busy 4;0",
            osc_title: "",
            osc_progress: "",
        };
        assert_eq!(detect_with(&manifest, screen_only).state, AgentState::Idle);
        let title = DetectionInput {
            screen: "",
            osc_title: "busy",
            osc_progress: "4;0",
        };
        assert_eq!(detect_with(&manifest, title).state, AgentState::Working);
    }

    #[test]
    fn skip_rule_suppresses_state_update() {
        let manifest = rules_manifest(
            r#"
[[rules]]
id = "activity"
state = "working"
priority = 10
visible_working = true
contains = ["activity-marker"]

[[rules]]
id = "overlay"
state = "unknown"
priority = 20
skip_state_update = true
contains = ["overlay-marker"]
"#,
        );
        let detection = detect_with(&manifest, input("activity-marker overlay-marker"));
        assert_eq!(detection.state, AgentState::Unknown);
        assert!(detection.skip_state_update);
        assert!(!detection.visible_working);
    }

    #[test]
    fn screen_regions_extract_structure() {
        for (screen, spec, expected) in [
            ("old\n\nnew\n", "bottom_lines(2)", "\nnew\n"),
            (
                "before\n› input\nafter\n",
                "after_last_prompt_marker",
                "after\n",
            ),
            (
                "before\n› input\nafter\n",
                "before_current_prompt_marker",
                "before\n",
            ),
            (
                "before\n› input\nafter\n",
                "whole_recent_without_current_prompt_marker",
                "",
            ),
            (
                "no marker\n",
                "whole_recent_without_current_prompt_marker",
                "no marker\n",
            ),
            (
                "• old\n■ latest\n› input\n",
                "current_prompt_block_marker",
                "■ latest",
            ),
            (
                "• old\n■ latest\n› input\n",
                "after_current_prompt_block_marker",
                "■ latest\n› input\n",
            ),
            ("› old\n• new\n", "current_prompt_block_marker", ""),
            (
                "above\n\n───\nbody\n───\nfooter\n",
                "above_prompt_box",
                "above\n\n",
            ),
            (
                "above\n\n───\nbody\n───\nfooter\n",
                "last_non_empty_above_prompt_box",
                "above",
            ),
            (
                "above\n───\nbody\n───\nfooter\n",
                "prompt_box_body",
                "body\n",
            ),
            (
                "above\n───\nbody\n───\nfooter\n",
                "after_last_horizontal_rule",
                "footer\n",
            ),
            (
                "marker\nold\n\nmiddle\nmarker\nnew\n",
                "bottom_non_empty_lines(2)",
                "marker\nnew\n",
            ),
            (
                "\nmarker\nold\n\nmiddle\nmarker\nnew\n",
                "top_non_empty_lines(2)",
                "\nmarker\nold\n",
            ),
        ] {
            assert_eq!(region(input(screen), spec), expected, "region={spec}");
        }
    }

    #[test]
    fn validation_rejects_bad_manifests() {
        for manifest in [
            "id = \"codex\"\n[[rules]]\nid = \"typo\"\nstate = \"working\"\ncontain = [\"Working\"]",
            "id = \"codex\"\n[[rules]]\nid = \"empty\"\nstate = \"working\"",
            "id = \"codex\"\n[[rules]]\nid = \"bad_region\"\nstate = \"working\"\nregion = \"after_last_promt_marker\"\ncontains = [\"Working\"]",
            "id = \"codex\"\n[[rules]]\nid = \"bad_regex\"\nstate = \"working\"\nregex = [\"[\"]",
            "id = \"codex\"\n[[rules]]\nid = \"bad_nested\"\nstate = \"working\"\nany = [{ line_regex = [\"[\"] }]",
            "id = \"codex\"\n[[rules]]\nid = \"bad_skip\"\nstate = \"idle\"\nskip_state_update = true\ncontains = [\"menu\"]",
            "id = \"codex\"\n[[rules]]\nid = \"bad_skip_visible\"\nstate = \"unknown\"\nskip_state_update = true\nvisible_blocker = true\ncontains = [\"menu\"]",
            "id = \"codex\"\nversion = \"1\"\nmin_engine_version = 2\n[[rules]]\nid = \"top\"\nstate = \"working\"\nregion = \" top_non_empty_lines(1) \"\ncontains = [\"active\"]",
        ] {
            assert!(parse_manifest(manifest).is_err(), "accepted:\n{manifest}");
        }

        let matchers = (0..33)
            .map(|index| format!("\"m{index}\""))
            .collect::<Vec<_>>()
            .join(", ");
        assert!(
            parse_manifest(&format!(
                "id = \"codex\"\n[[rules]]\nid = \"many\"\nstate = \"idle\"\ncontains = [{matchers}]"
            ))
            .is_err()
        );
        for count in ["0", "01", "+1", "65536"] {
            assert!(validate_region_name(&format!("top_non_empty_lines({count})")).is_err());
        }
    }

    #[test]
    fn bundled_manifests_read_real_screens() {
        let claude_permission = "\
 Bash command

   rm -rf build

 Do you want to proceed?
 ❯ 1. Yes
   2. Yes, and don't ask again for rm commands in this project
   3. No, and tell Claude what to do differently (esc)
";
        let detection = detect(Agent::Claude, input(claude_permission));
        assert_eq!(detection.state, AgentState::Blocked);
        assert!(detection.visible_blocker);

        let claude_working = "\
✻ Thinking… (12s · ↑ 1.2k tokens · esc to interrupt)

────────────────────────────────────────
❯
────────────────────────────────────────
  ⏵⏵ accept edits on (shift+tab to cycle)
";
        assert_eq!(
            detect(Agent::Claude, input(claude_working)).state,
            AgentState::Working
        );

        let claude_idle = "\
● Done.

────────────────────────────────────────
❯
────────────────────────────────────────
  ? for shortcuts
";
        let detection = detect(Agent::Claude, input(claude_idle));
        assert_eq!(detection.state, AgentState::Idle);
        assert!(detection.visible_idle);

        let codex_title = DetectionInput {
            screen: "",
            osc_title: "⠋ project",
            osc_progress: "",
        };
        assert_eq!(detect(Agent::Codex, codex_title).state, AgentState::Working);
    }
}
