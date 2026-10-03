//! Folders on this machine, for adding a project here from another one (t3code's
//! `filesystem.browse`).

use std::path::{Path, PathBuf};

use agentz_protocol::{DirectoryEntry, DirectoryListing};
use anyhow::{Context as _, Result, anyhow};

/// Expands a leading `~` to this machine's home.
pub(crate) fn expand_home(path: &Path) -> PathBuf {
    let Ok(rest) = path.strip_prefix("~") else {
        return path.to_path_buf();
    };
    util::paths::home_dir().join(rest)
}

pub(crate) fn browse(partial_path: &str) -> Result<DirectoryListing> {
    let partial_path = match partial_path.trim() {
        "" => "~/",
        partial_path => partial_path,
    };
    let resolved = expand_home(Path::new(partial_path));
    if !resolved.is_absolute() {
        return Err(anyhow!(
            "{partial_path} isn't a full path; start it with / or ~"
        ));
    }
    let ends_with_separator = partial_path.ends_with('/') || partial_path == "~";
    let (parent, prefix) = if ends_with_separator {
        (resolved, String::new())
    } else {
        // Split the text, not the `Path`: `Path` drops a trailing `.`, which is
        // exactly how someone starts typing a hidden folder's name.
        let resolved = resolved.to_string_lossy();
        let (parent, prefix) = resolved.rsplit_once('/').unwrap_or(("", &resolved));
        let parent = match parent {
            "" => PathBuf::from("/"),
            parent => PathBuf::from(parent),
        };
        (parent, prefix.to_string())
    };
    let read = match std::fs::read_dir(&parent) {
        Ok(read) => read,
        // A folder that can't be read just has nothing to offer.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Ok(DirectoryListing {
                parent,
                entries: Vec::new(),
            });
        }
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", parent.display()));
        }
    };
    let show_hidden = ends_with_separator || prefix.starts_with('.');
    let lower_prefix = prefix.to_lowercase();
    let mut entries: Vec<DirectoryEntry> = read
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let matches = name.to_lowercase().starts_with(&lower_prefix)
                && (show_hidden || !name.starts_with('.'));
            // Follows symlinks, so a linked folder counts.
            (matches && path.is_dir()).then_some(DirectoryEntry { name, path })
        })
        .collect();
    // Hidden folders go last: a project is rarely one, and home has dozens.
    entries.sort_by_cached_key(|entry| (entry.name.starts_with('.'), entry.name.to_lowercase()));
    Ok(DirectoryListing { parent, entries })
}

#[cfg(test)]
mod tests {
    use super::browse;

    #[test]
    fn browses_like_t3code() {
        let root = tempfile::tempdir().expect("temp dir");
        for name in ["alpha", "Apple", "beta", ".hidden"] {
            std::fs::create_dir(root.path().join(name)).expect("folder");
        }
        std::fs::write(root.path().join("afile"), "").expect("file");
        let root_path = root.path().display().to_string();
        let names = |partial: &str| -> Vec<String> {
            browse(partial)
                .expect("browses")
                .entries
                .into_iter()
                .map(|entry| entry.name)
                .collect()
        };

        assert_eq!(
            names(&format!("{root_path}/")),
            ["alpha", "Apple", "beta", ".hidden"]
        );
        assert_eq!(names(&format!("{root_path}/a")), ["alpha", "Apple"]);
        assert_eq!(names(&format!("{root_path}/.")), [".hidden"]);
        assert_eq!(names(&format!("{root_path}/zzz")), Vec::<String>::new());
        let listing = browse(&format!("{root_path}/be")).expect("browses");
        assert_eq!(listing.parent, root.path());
        assert_eq!(listing.entries[0].path, root.path().join("beta"));
        assert!(browse("relative/path").is_err());
        assert!(browse(&format!("{root_path}/missing/")).is_err());
    }
}
