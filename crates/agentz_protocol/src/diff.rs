//! A thread's changes, from the checkpoints the server takes around each turn (t3code's).

use serde::{Deserialize, Serialize};

use crate::thread::DiffLineKind;

/// Which changes [`crate::Request::ThreadDiff`] asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffScope {
    /// The latest finished turn's.
    LatestTurn,
    /// Everything since the thread's first turn started.
    #[default]
    All,
    /// One finished turn's, by its number (the first is 1).
    Turn(u32),
    /// The folder's uncommitted changes, untracked files included: t3code's "Working tree",
    /// as the folder is now.
    WorkingTree,
    /// The branch's commits since it left its base branch: t3code's "Branch changes".
    Branch,
}

impl DiffScope {
    /// Whether it's of the turns' checkpoints, rather than of the folder as it is.
    pub fn is_turns(self) -> bool {
        matches!(
            self,
            DiffScope::LatestTurn | DiffScope::All | DiffScope::Turn(_)
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadDiff {
    pub status: DiffStatus,
    /// How many turns have finished with a checkpoint.
    pub turns: u32,
    pub files: Vec<DiffFile>,
    /// The patch was too big, so the last files are missing or cut short.
    pub truncated: bool,
    #[serde(default)]
    pub restore: RestoreAvailability,
    /// The finished turns, oldest first, for picking one.
    #[serde(default)]
    pub finished_turns: Vec<FinishedTurn>,
    /// What [`DiffScope::Branch`] compared with, such as `origin/main`; `None` when the branch has
    /// no base to compare with.
    #[serde(default)]
    pub base_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FinishedTurn {
    pub number: u32,
    pub finished_at: Option<std::time::SystemTime>,
}

/// Whether [`crate::Request::RestoreCheckpoint`] can put the thread's files back. A checkpoint
/// saves the whole folder, so only a thread alone in its own worktree or pasture may restore
/// (t3code).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RestoreAvailability {
    Available,
    /// Why not.
    Unavailable(String),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl Default for RestoreAvailability {
    fn default() -> Self {
        RestoreAvailability::Unavailable("This server can't restore files.".into())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum DiffStatus {
    #[default]
    Ready,
    /// The thread's folder isn't in a git repository, so it has no checkpoints.
    NotRepository,
    /// No turn has finished yet.
    NoTurns,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffFile {
    /// Relative to the repository, as git prints it.
    pub path: String,
    /// The path before a rename.
    pub old_path: Option<String>,
    pub change: FileChange,
    pub binary: bool,
    pub additions: u32,
    pub deletions: u32,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChange {
    Added,
    Deleted,
    Modified,
    Renamed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffHunk {
    /// The `@@ -1,3 +1,4 @@` line, with any function context git adds.
    pub header: String,
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// Reads `git diff --patch` output made with `--src-prefix=a/ --dst-prefix=b/` and
/// `core.quotePath=false`. A patch cut short ends with whatever its last file had.
pub fn parse_patch(patch: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("diff --git ") {
            let path = same_path_from_header(header).unwrap_or_default();
            files.push(DiffFile {
                path: path.clone(),
                old_path: None,
                change: FileChange::Modified,
                binary: false,
                additions: 0,
                deletions: 0,
                hunks: Vec::new(),
            });
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if let Some(hunk) = file.hunks.last_mut() {
            let (kind, text) = match line.as_bytes().first() {
                Some(b'+') => (DiffLineKind::Added, &line[1..]),
                Some(b'-') => (DiffLineKind::Removed, &line[1..]),
                Some(b' ') => (DiffLineKind::Context, &line[1..]),
                // "\ No newline at end of file"
                Some(b'\\') => continue,
                _ => {
                    if line.starts_with("@@") {
                        if let Some(hunk) = parse_hunk_header(line) {
                            file.hunks.push(hunk);
                        }
                    }
                    continue;
                }
            };
            match kind {
                DiffLineKind::Added => file.additions += 1,
                DiffLineKind::Removed => file.deletions += 1,
                DiffLineKind::Context => {}
            }
            hunk.lines.push(DiffLine {
                kind,
                text: text.to_string(),
            });
            continue;
        }
        if line.starts_with("@@") {
            if let Some(hunk) = parse_hunk_header(line) {
                file.hunks.push(hunk);
            }
        } else if line.starts_with("new file mode") {
            file.change = FileChange::Added;
        } else if line.starts_with("deleted file mode") {
            file.change = FileChange::Deleted;
        } else if let Some(path) = line.strip_prefix("rename from ") {
            file.change = FileChange::Renamed;
            file.old_path = Some(unquote(path));
        } else if let Some(path) = line.strip_prefix("rename to ") {
            file.change = FileChange::Renamed;
            file.path = unquote(path);
        } else if let Some(path) = line.strip_prefix("--- ") {
            if file.change != FileChange::Renamed
                && let Some(path) = strip_side(path, "a/")
            {
                file.path = path;
            }
        } else if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(path) = strip_side(path, "b/") {
                file.path = path;
            }
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            file.binary = true;
        }
    }
    files
}

/// `a/<path> b/<path>` names the path twice when the file wasn't renamed, so it's the first
/// half, however many spaces it has.
fn same_path_from_header(header: &str) -> Option<String> {
    if header.starts_with('"') {
        let (first, rest) = split_quoted(header)?;
        let second = rest.strip_prefix(' ')?;
        let second = if second.starts_with('"') {
            split_quoted(second)?.0
        } else {
            second.to_string()
        };
        return (first.strip_prefix("a/")? == second.strip_prefix("b/")?)
            .then(|| first["a/".len()..].to_string());
    }
    let length = header.len().checked_sub(" ".len())?;
    if length % 2 != 0 {
        return None;
    }
    let (first, second) = header.split_at(length / 2);
    let second = second.strip_prefix(' ')?;
    let path = first.strip_prefix("a/")?;
    (second.strip_prefix("b/")? == path).then(|| path.to_string())
}

/// `a/<path>` or `/dev/null` (`None`), from a `---` or `+++` line.
fn strip_side(path: &str, prefix: &str) -> Option<String> {
    let path = path.strip_suffix('\t').unwrap_or(path);
    if path == "/dev/null" {
        return None;
    }
    let path = unquote(path);
    path.strip_prefix(prefix).map(str::to_string)
}

fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    let ranges = line.strip_prefix("@@ ")?;
    let (ranges, _) = ranges.split_once(" @@")?;
    let (old, new) = ranges.split_once(' ')?;
    let start = |range: &str, sign: char| -> Option<u32> {
        range.strip_prefix(sign)?.split(',').next()?.parse().ok()
    };
    Some(DiffHunk {
        header: line.to_string(),
        old_start: start(old, '-')?,
        new_start: start(new, '+')?,
        lines: Vec::new(),
    })
}

/// Git quotes paths with special characters in C style: `"a/tab\there"`.
fn unquote(path: &str) -> String {
    if path.starts_with('"')
        && let Some((unquoted, _)) = split_quoted(path)
    {
        return unquoted;
    }
    path.to_string()
}

/// The quoted string at the start of `text`, unescaped, and what follows it.
fn split_quoted(text: &str) -> Option<(String, &str)> {
    let mut bytes = Vec::new();
    let mut chars = text.strip_prefix('"')?.char_indices();
    while let Some((index, character)) = chars.next() {
        match character {
            '"' => {
                let rest = &text[1 + index + 1..];
                return Some((String::from_utf8_lossy(&bytes).into_owned(), rest));
            }
            '\\' => {
                let (_, escaped) = chars.next()?;
                match escaped {
                    'n' => bytes.push(b'\n'),
                    't' => bytes.push(b'\t'),
                    'r' => bytes.push(b'\r'),
                    'a' => bytes.push(7),
                    'b' => bytes.push(8),
                    'f' => bytes.push(12),
                    'v' => bytes.push(11),
                    '0'..='7' => {
                        let mut value = escaped.to_digit(8)?;
                        for _ in 0..2 {
                            let (_, digit) = chars.next()?;
                            value = value * 8 + digit.to_digit(8)?;
                        }
                        bytes.push(u8::try_from(value).ok()?);
                    }
                    other => {
                        let mut buffer = [0; 4];
                        bytes.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
                    }
                }
            }
            other => {
                let mut buffer = [0; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches() {
        let patch = "\
diff --git a/src/my file.rs b/src/my file.rs
index 1111111..2222222 100644
--- a/src/my file.rs
+++ b/src/my file.rs
@@ -1,3 +1,4 @@ fn main() {
 one
-two
+TWO
+three
 four
\\ No newline at end of file
diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/new.txt
@@ -0,0 +1 @@
+hello
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index 4444444..0000000
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
diff --git a/old name.txt b/new name.txt
similarity 100%
rename from old name.txt
rename to new name.txt
diff --git a/logo.png b/logo.png
new file mode 100644
index 0000000..5555555
Binary files /dev/null and b/logo.png differ
diff --git \"a/tab\\there\" \"b/tab\\there\"
new file mode 100644
index 0000000..6666666
--- /dev/null
+++ \"b/tab\\there\"
@@ -0,0 +1 @@
+x
";
        let files = parse_patch(patch);
        let summary: Vec<(&str, Option<&str>, FileChange, bool, u32, u32)> = files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.old_path.as_deref(),
                    file.change,
                    file.binary,
                    file.additions,
                    file.deletions,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("src/my file.rs", None, FileChange::Modified, false, 2, 1),
                ("new.txt", None, FileChange::Added, false, 1, 0),
                ("gone.txt", None, FileChange::Deleted, false, 0, 1),
                (
                    "new name.txt",
                    Some("old name.txt"),
                    FileChange::Renamed,
                    false,
                    0,
                    0
                ),
                ("logo.png", None, FileChange::Added, true, 0, 0),
                ("tab\there", None, FileChange::Added, false, 1, 0),
            ]
        );
        let hunk = &files[0].hunks[0];
        assert_eq!((hunk.old_start, hunk.new_start), (1, 1));
        assert_eq!(hunk.header, "@@ -1,3 +1,4 @@ fn main() {");
        let lines: Vec<(DiffLineKind, &str)> = hunk
            .lines
            .iter()
            .map(|line| (line.kind, line.text.as_str()))
            .collect();
        assert_eq!(
            lines,
            vec![
                (DiffLineKind::Context, "one"),
                (DiffLineKind::Removed, "two"),
                (DiffLineKind::Added, "TWO"),
                (DiffLineKind::Added, "three"),
                (DiffLineKind::Context, "four"),
            ]
        );
    }
}
