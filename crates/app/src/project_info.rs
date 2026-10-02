//! What t3code's sidebar shows about a project besides its name: an icon (the project's
//! favicon, or a colored monogram when it has none) and the checked-out git branch.

use std::path::{Path, PathBuf};
use std::time::Duration;

use collections::HashMap;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, Global, Hsla, Subscription,
    Task, img, rgb,
};
use projects::{Project, ProjectIcon, ProjectId, ProjectStore};
use ui::{StyledImage as _, prelude::*};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectInfo {
    pub favicon: Option<PathBuf>,
    pub git_head: Option<GitHead>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GitHead {
    /// The branch name, or the abbreviated commit when HEAD is detached.
    pub branch: String,
    /// Set when the project folder is a linked worktree rather than the main checkout.
    pub worktree: Option<PathBuf>,
}

impl ProjectInfo {
    /// Reads the project's files, so callers run it off the main thread.
    pub fn read(root: &Path) -> Self {
        Self {
            favicon: find_favicon(root),
            git_head: read_git_head(root),
        }
    }
}

/// How often icons and checked-out branches are re-read.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

/// Every project's [`ProjectInfo`], kept current for the sidebar, the project switcher and
/// settings.
pub struct ProjectInfoStore {
    info: HashMap<ProjectId, ProjectInfo>,
    refresh: Task<()>,
    _projects_subscription: Subscription,
}

struct GlobalProjectInfo(Entity<ProjectInfoStore>);

impl Global for GlobalProjectInfo {}

/// Call after `projects::init`.
pub fn init(cx: &mut App) {
    let projects = ProjectStore::global(cx);
    let store = cx.new(|cx| {
        let subscription = cx.observe(&projects, |this: &mut ProjectInfoStore, projects, cx| {
            // Added or removed projects shouldn't wait for the next refresh.
            let current = projects.read(cx).projects();
            let is_stale = current.len() != this.info.len()
                || current
                    .iter()
                    .any(|project| !this.info.contains_key(&project.id));
            if is_stale {
                this.refresh = ProjectInfoStore::refresh_loop(projects, cx);
            }
        });
        ProjectInfoStore {
            info: HashMap::default(),
            refresh: ProjectInfoStore::refresh_loop(projects.clone(), cx),
            _projects_subscription: subscription,
        }
    });
    cx.set_global(GlobalProjectInfo(store));
}

impl ProjectInfoStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalProjectInfo>().0.clone()
    }

    pub fn info(&self) -> &HashMap<ProjectId, ProjectInfo> {
        &self.info
    }

    fn refresh_loop(projects: Entity<ProjectStore>, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let roots: Vec<(ProjectId, PathBuf)> = projects.read_with(cx, |projects, _| {
                    projects
                        .projects()
                        .iter()
                        .map(|project| (project.id, project.path.clone()))
                        .collect()
                });
                let info = cx
                    .background_spawn(async move {
                        roots
                            .into_iter()
                            .map(|(id, root)| (id, ProjectInfo::read(&root)))
                            .collect::<HashMap<_, _>>()
                    })
                    .await;
                let updated = this.update(cx, |this, cx| {
                    if this.info != info {
                        this.info = info;
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
                cx.background_executor().timer(REFRESH_INTERVAL).await;
            }
        })
    }
}

/// Well-known favicon paths, checked in order (t3code's list).
const FAVICON_CANDIDATES: &[&str] = &[
    "favicon.svg",
    "favicon.ico",
    "favicon.png",
    "public/favicon.svg",
    "public/favicon.ico",
    "public/favicon.png",
    "app/favicon.ico",
    "app/favicon.png",
    "app/icon.svg",
    "app/icon.png",
    "app/icon.ico",
    "src/favicon.ico",
    "src/favicon.svg",
    "src/app/favicon.ico",
    "src/app/icon.svg",
    "src/app/icon.png",
    "assets/icon.svg",
    "assets/icon.png",
    "assets/logo.svg",
    "assets/logo.png",
    ".idea/icon.svg",
];

/// Files that may declare the icon with a `<link rel="icon">` tag or `{ rel: "icon" }` metadata.
const ICON_SOURCE_FILES: &[&str] = &[
    "index.html",
    "public/index.html",
    "app/routes/__root.tsx",
    "src/routes/__root.tsx",
    "app/root.tsx",
    "src/root.tsx",
    "src/index.html",
];

fn find_favicon(root: &Path) -> Option<PathBuf> {
    let existing = |relative: &str| {
        let path = root.join(relative);
        path.is_file().then_some(path)
    };
    if let Some(path) = FAVICON_CANDIDATES
        .iter()
        .find_map(|relative| existing(relative))
    {
        return Some(path);
    }
    ICON_SOURCE_FILES.iter().find_map(|relative| {
        let source = std::fs::read_to_string(root.join(relative)).ok()?;
        let href = icon_href(&source)?;
        let href = href.trim_start_matches('/');
        existing(&format!("public/{href}")).or_else(|| existing(href))
    })
}

/// Finds the icon's `href` in a `<link>` tag or in object-like metadata, with `rel` and `href`
/// in either order.
fn icon_href(source: &str) -> Option<&str> {
    // ASCII lowercasing keeps byte offsets, so positions found in `lower` index `source`.
    let lower = source.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(offset) = lower.get(search_from..)?.find("<link") {
        let start = search_from + offset;
        let end = lower[start..]
            .find('>')
            .map_or(lower.len(), |end| start + end);
        if has_icon_rel(&lower[start..end], '=')
            && let Some(href) = attribute_value(&source[start..end], "href", '=')
        {
            return Some(href);
        }
        search_from = end;
    }
    let mut run_start = 0;
    for run in source.split('}') {
        let run_lower = &lower[run_start..run_start + run.len()];
        if has_icon_rel(run_lower, ':')
            && let Some(href) = attribute_value(run, "href", ':')
        {
            return Some(href);
        }
        run_start += run.len() + 1;
    }
    None
}

fn has_icon_rel(text_lower: &str, separator: char) -> bool {
    let mut rest = text_lower;
    while let Some(value) = attribute_value(rest, "rel", separator) {
        if value == "icon" || value == "shortcut icon" {
            return true;
        }
        let Some(position) = rest.find(value) else {
            break;
        };
        rest = &rest[position + value.len()..];
    }
    false
}

/// The quoted value after `name` and `separator` (`href="…"` or `href: "…"`), up to the closing
/// quote or a `?`.
fn attribute_value<'a>(text: &'a str, name: &str, separator: char) -> Option<&'a str> {
    let mut search_from = 0;
    while let Some(offset) = text.get(search_from..)?.find(name) {
        let start = search_from + offset;
        search_from = start + name.len();
        let starts_word = text[..start]
            .chars()
            .next_back()
            .is_none_or(|previous| !previous.is_alphanumeric() && previous != '-');
        if !starts_word {
            continue;
        }
        let rest = text[start + name.len()..].trim_start();
        let Some(rest) = rest.strip_prefix(separator) else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let value = &rest[quote.len_utf8()..];
        let end = value
            .find(|c: char| c == quote || c == '?')
            .unwrap_or(value.len());
        return Some(&value[..end]);
    }
    None
}

/// Reads HEAD from the repository containing `root`, which may be a subfolder of it.
fn read_git_head(root: &Path) -> Option<GitHead> {
    let (checkout, dot_git) = root.ancestors().find_map(|directory| {
        let dot_git = directory.join(".git");
        dot_git.exists().then(|| (directory.to_path_buf(), dot_git))
    })?;
    let (git_dir, worktree) = if dot_git.is_dir() {
        (dot_git, None)
    } else {
        // A linked worktree's `.git` is a file pointing at its git directory.
        let contents = std::fs::read_to_string(&dot_git).ok()?;
        let git_dir = PathBuf::from(contents.trim().strip_prefix("gitdir:")?.trim());
        (checkout.join(git_dir), Some(checkout))
    };
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    Some(GitHead {
        branch: branch_from_head(&head)?,
        worktree,
    })
}

fn branch_from_head(head: &str) -> Option<String> {
    let head = head.trim();
    if head.is_empty() {
        return None;
    }
    Some(match head.strip_prefix("ref:") {
        Some(reference) => {
            let reference = reference.trim();
            reference
                .strip_prefix("refs/heads/")
                .unwrap_or(reference)
                .to_string()
        }
        None => head.chars().take(7).collect(),
    })
}

/// The icon picked in the project's settings, else its favicon, else t3code's monogram tile
/// (also shown when an image fails to load).
pub fn render_project_icon(
    project: &Project,
    info: Option<&ProjectInfo>,
    size: Pixels,
    cx: &App,
) -> AnyElement {
    let name = project.name();
    let is_light = cx.theme().appearance().is_light();
    let font_family = theme::theme_settings(cx).buffer_font(cx).family.clone();
    let (text, color, image) = match &project.icon {
        Some(ProjectIcon::Monogram { text, color }) => {
            let color = MONOGRAM_COLORS
                .iter()
                .find(|(name, _, _)| name == color)
                .map(|(_, light, dark)| rgb(if is_light { *light } else { *dark }).into())
                .unwrap_or_else(|| monogram_color(&name, is_light));
            let text: String = text.chars().take(2).collect();
            return render_monogram(text.to_uppercase().into(), color, font_family, size)
                .into_any_element();
        }
        Some(ProjectIcon::Image { path }) => (
            monogram(&name),
            monogram_color(&name, is_light),
            Some(path.clone()),
        ),
        None => (
            monogram(&name),
            monogram_color(&name, is_light),
            info.and_then(|info| info.favicon.clone()),
        ),
    };
    let text = SharedString::from(text);
    match image {
        Some(favicon) => img(favicon)
            .size(size)
            .flex_none()
            .rounded_sm()
            .with_fallback(move || {
                render_monogram(text.clone(), color, font_family.clone(), size).into_any_element()
            })
            .into_any_element(),
        None => render_monogram(text, color, font_family, size).into_any_element(),
    }
}

/// t3code draws the monogram on a 16px tile with 8.25px text and a 25% corner radius.
fn render_monogram(
    text: SharedString,
    color: Hsla,
    font_family: SharedString,
    size: Pixels,
) -> Div {
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(size * 0.25)
        .bg(color.opacity(0.14))
        .text_color(color)
        .text_size(size * (8.25 / 16.))
        .line_height(size)
        .font_weight(FontWeight::BOLD)
        .font_family(font_family)
        .child(text)
}

/// Two letters for a project: the first letter, then a digit from the first word, the first
/// letter of the last word, or the first word's last letter (t3code's rule).
fn monogram(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let Some(first_glyph) = words.first().and_then(|word| word.chars().next()) else {
        return "PR".to_string();
    };
    let first_word: Vec<char> = words
        .first()
        .map(|word| word.chars().collect())
        .unwrap_or_default();
    let second_glyph = first_word
        .iter()
        .skip(1)
        .find(|glyph| glyph.is_numeric())
        .copied()
        .or_else(|| {
            if words.len() > 1 {
                words.last().and_then(|word| word.chars().next())
            } else {
                first_word.last().copied()
            }
        })
        .unwrap_or(first_glyph);
    format!("{first_glyph}{second_glyph}")
        .to_uppercase()
        .chars()
        .take(2)
        .collect()
}

/// t3code's project colors: Tailwind's 600 shades on light themes and 400 shades on dark ones.
pub const MONOGRAM_COLORS: [(&str, u32, u32); 18] = [
    ("gray", 0x4b5563, 0x9ca3af),
    ("red", 0xdc2626, 0xf87171),
    ("orange", 0xea580c, 0xfb923c),
    ("amber", 0xd97706, 0xfbbf24),
    ("yellow", 0xca8a04, 0xfacc15),
    ("lime", 0x65a30d, 0xa3e635),
    ("green", 0x16a34a, 0x4ade80),
    ("emerald", 0x059669, 0x34d399),
    ("teal", 0x0d9488, 0x2dd4bf),
    ("cyan", 0x0891b2, 0x22d3ee),
    ("sky", 0x0284c7, 0x38bdf8),
    ("blue", 0x2563eb, 0x60a5fa),
    ("indigo", 0x4f46e5, 0x818cf8),
    ("violet", 0x7c3aed, 0xa78bfa),
    ("purple", 0x9333ea, 0xc084fc),
    ("fuchsia", 0xc026d3, 0xe879f9),
    ("pink", 0xdb2777, 0xf472b6),
    ("rose", 0xe11d48, 0xfb7185),
];

/// The named color in the current theme's shade.
pub fn monogram_swatch(light: u32, dark: u32, cx: &App) -> Hsla {
    rgb(if cx.theme().appearance().is_light() {
        light
    } else {
        dark
    })
    .into()
}

fn monogram_color(name: &str, is_light: bool) -> Hsla {
    let (_, light, dark) = MONOGRAM_COLORS
        .get(monogram_color_index(name))
        .copied()
        .unwrap_or(("blue", 0x2563eb, 0x60a5fa));
    rgb(if is_light { light } else { dark }).into()
}

/// The monogram and color name a project gets automatically, as a starting point for a
/// custom one.
pub fn automatic_monogram(name: &str) -> (String, &'static str) {
    let color = MONOGRAM_COLORS
        .get(monogram_color_index(name))
        .map_or("blue", |(color, _, _)| color);
    (monogram(name), color)
}

/// A stable color per project name, hashed the way t3code does.
fn monogram_color_index(name: &str) -> usize {
    let seed = name.trim().to_lowercase();
    let seed = if seed.is_empty() { "project" } else { &seed };
    seed.chars().fold(0, |index, glyph| {
        (index * 31 + glyph as usize) % MONOGRAM_COLORS.len()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monograms() {
        assert_eq!(monogram("dramabot"), "DT");
        assert_eq!(monogram("shop-landing"), "SL");
        assert_eq!(monogram("web3app"), "W3");
        assert_eq!(monogram("x"), "XX");
        assert_eq!(monogram("--"), "PR");
    }

    #[test]
    fn monogram_colors_are_stable() {
        assert_eq!(
            monogram_color_index("agentZ"),
            monogram_color_index("AGENTZ")
        );
        assert!(monogram_color_index("anything") < MONOGRAM_COLORS.len());
    }

    #[test]
    fn icon_links() {
        assert_eq!(
            icon_href(
                r#"<head><link rel="stylesheet" href="a.css"><link href="/logo.svg?v=2" rel="icon"></head>"#
            ),
            Some("/logo.svg")
        );
        assert_eq!(
            icon_href(
                r#"links: () => [{ rel: 'stylesheet', href: 'a.css' }, { rel: "icon", href: "/favicon.png" }]"#
            ),
            Some("/favicon.png")
        );
        assert_eq!(
            icon_href(r#"<link rel="shortcut icon" href='fav.ico'>"#),
            Some("fav.ico")
        );
        assert_eq!(icon_href("<p>no icon</p>"), None);
    }

    #[test]
    fn branches() {
        assert_eq!(
            branch_from_head("ref: refs/heads/main\n").as_deref(),
            Some("main")
        );
        assert_eq!(
            branch_from_head("ref: refs/heads/feature/sidebar").as_deref(),
            Some("feature/sidebar")
        );
        assert_eq!(
            branch_from_head("57bfce2945aa\n").as_deref(),
            Some("57bfce2")
        );
        assert_eq!(branch_from_head(""), None);
    }

    #[test]
    fn reads_this_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(read_git_head(root).is_some());
    }
}
