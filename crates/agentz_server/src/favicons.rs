//! A project's icon, found in its folder as t3code's `ProjectFaviconResolver` finds one:
//! well-known favicon files first, then the icon a page or a route declares. Each machine's
//! server looks in its own projects' folders, so projects on other machines than the app's
//! show theirs too.

use std::path::{Component, Path, PathBuf};

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

/// The project's icon file, if it has one. Reads the folder, so it runs off the server's task.
pub(crate) fn find(root: &Path) -> Option<PathBuf> {
    let existing = |relative: &str| {
        let path = root.join(within_root(relative)?);
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
        let href = href.strip_prefix('/').unwrap_or(href);
        existing(&format!("public/{href}")).or_else(|| existing(href))
    })
}

/// `relative` as a path inside the project's folder, or `None` when it's absolute or leads out
/// of it: a page's icon link can't name a file elsewhere on the machine.
fn within_root(relative: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for component in Path::new(relative.trim()).components() {
        match component {
            Component::Normal(name) => path.push(name),
            Component::CurDir => {}
            Component::ParentDir => {
                if !path.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!path.as_os_str().is_empty()).then_some(path)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a folder");
        }
        std::fs::write(path, text).expect("written");
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
        assert_eq!(
            icon_href(
                r#"const links = [{ rel: "icon" }, { rel: "icon", href: "/brand/logo.svg" }];"#
            ),
            Some("/brand/logo.svg")
        );
        assert_eq!(icon_href("<p>no icon</p>"), None);
    }

    #[test]
    fn well_known_files_come_first_in_order() {
        let root = tempfile::tempdir().expect("a temporary folder");
        let root = root.path();
        assert_eq!(find(root), None);
        write(
            root,
            "index.html",
            r#"<link rel="icon" href="/brand/logo.svg">"#,
        );
        write(root, "public/brand/logo.svg", "<svg/>");
        assert_eq!(find(root), Some(root.join("public/brand/logo.svg")));
        write(root, "public/favicon.png", "png");
        assert_eq!(find(root), Some(root.join("public/favicon.png")));
        write(root, "favicon.ico", "ico");
        assert_eq!(find(root), Some(root.join("favicon.ico")));
    }

    #[test]
    fn declared_icons_stay_in_the_project() {
        let root = tempfile::tempdir().expect("a temporary folder");
        let project = root.path().join("project");
        write(root.path(), "secret.svg", "<svg/>");
        write(
            &project,
            "index.html",
            r#"<link rel="icon" href="../secret.svg">"#,
        );
        assert_eq!(find(&project), None);
        // A later source still counts.
        write(
            &project,
            "public/index.html",
            r#"<link rel="icon" href="/brand/logo.svg">"#,
        );
        write(&project, "public/brand/logo.svg", "<svg/>");
        assert_eq!(find(&project), Some(project.join("public/brand/logo.svg")));

        assert_eq!(
            within_root("a/./b/../c.svg"),
            Some(PathBuf::from("a/c.svg"))
        );
        assert_eq!(within_root("public/../../secret.svg"), None);
        assert_eq!(within_root("/etc/icon.svg"), None);
        assert_eq!(within_root("."), None);
    }
}
