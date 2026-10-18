//! agentZ's own SSH key for cloud machines. Every machine agentZ makes is given it, replacing
//! a machine's keys keeps it, and agentZ reaches the providers' machines with it, so making
//! one doesn't depend on the keys the user has in `~/.ssh`: Proton's CLI looks only for
//! `id_ed25519.pub`, `id_ecdsa.pub` and `id_rsa.pub` there. It's made with `ssh-keygen` the
//! first time a machine needs it, and never replaced, since machines have it.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};

const KEYGEN_TIMEOUT: Duration = Duration::from_secs(30);

/// The keys `ssh` offers by itself, which an `IdentityFile` option replaces: a machine made
/// outside agentZ has one of the user's own.
const SSH_DEFAULT_IDENTITIES: [&str; 5] = [
    "~/.ssh/id_rsa",
    "~/.ssh/id_ecdsa",
    "~/.ssh/id_ecdsa_sk",
    "~/.ssh/id_ed25519",
    "~/.ssh/id_ed25519_sk",
];

#[derive(Clone, Debug)]
pub struct OwnKey {
    /// The private key; the public one is beside it, with `.pub`.
    path: PathBuf,
    /// Held while the key is made, so machines made at once get the same one.
    making: Arc<tokio::sync::Mutex<()>>,
}

impl OwnKey {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            making: Arc::default(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn public_path(&self) -> PathBuf {
        with_suffix(&self.path, ".pub")
    }

    /// The public key's file, with the key made first if there's none yet.
    pub async fn public_key(&self) -> Result<PathBuf> {
        let _making = self.making.lock().await;
        let public = self.public_path();
        if exists(&self.path).await? {
            if !exists(&public).await? {
                // Machines may have the key, so only its public half is made again.
                let derived =
                    ssh_keygen([OsStr::new("-y"), OsStr::new("-f"), self.path.as_os_str()]).await?;
                tokio::fs::write(&public, derived)
                    .await
                    .with_context(|| format!("writing {}", public.display()))?;
            }
            return Ok(public);
        }
        let folder = self
            .path
            .parent()
            .with_context(|| format!("{} has no folder", self.path.display()))?;
        tokio::fs::create_dir_all(folder)
            .await
            .with_context(|| format!("making {}", folder.display()))?;
        // Made under another name and moved into place, private half last, so a key cut short
        // isn't taken for one.
        let made = with_suffix(&self.path, ".new");
        let made_public = with_suffix(&made, ".pub");
        for leftover in [&made, &made_public] {
            remove_if_there(leftover).await?;
        }
        ssh_keygen([
            OsStr::new("-q"),
            OsStr::new("-t"),
            OsStr::new("ed25519"),
            OsStr::new("-N"),
            OsStr::new(""),
            OsStr::new("-C"),
            OsStr::new("agentZ"),
            OsStr::new("-f"),
            made.as_os_str(),
        ])
        .await?;
        tokio::fs::rename(&made_public, &public)
            .await
            .with_context(|| format!("moving the new key to {}", public.display()))?;
        tokio::fs::rename(&made, &self.path)
            .await
            .with_context(|| format!("moving the new key to {}", self.path.display()))?;
        log::info!(
            "made agentZ's SSH key for cloud machines, {}",
            self.path.display()
        );
        Ok(public)
    }

    /// `IdentityFile` options for `ssh`: this key once it's made, then the ones `ssh` would
    /// have offered without them.
    pub async fn identity_options(&self) -> Result<Vec<(String, String)>> {
        if !exists(&self.path).await? {
            return Ok(Vec::new());
        }
        let own = self
            .path
            .to_str()
            .with_context(|| format!("{} isn't a path ssh can take", self.path.display()))?;
        Ok(std::iter::once(own)
            .chain(SSH_DEFAULT_IDENTITIES)
            .map(|path| ("IdentityFile".to_string(), path.to_string()))
            .collect())
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut path = OsString::from(path.as_os_str());
    path.push(suffix);
    PathBuf::from(path)
}

async fn exists(path: &Path) -> Result<bool> {
    tokio::fs::try_exists(path)
        .await
        .with_context(|| format!("looking for {}", path.display()))
}

async fn remove_if_there(path: &Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("removing {}", path.display()))
        }
        _ => Ok(()),
    }
}

async fn ssh_keygen<'a>(args: impl IntoIterator<Item = &'a OsStr>) -> Result<Vec<u8>> {
    let mut command = tokio::process::Command::new("ssh-keygen");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(KEYGEN_TIMEOUT, command.output())
        .await
        .context("ssh-keygen didn't finish")?
        .context("running ssh-keygen")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "ssh-keygen failed: {}",
            stderr.trim().lines().last().unwrap_or("no message")
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[tokio::test]
    async fn makes_the_key_once_and_keeps_it() {
        let folder = tempfile::tempdir().expect("makes a folder");
        let key = OwnKey::new(folder.path().join("cloud").join("id_ed25519"));
        assert!(
            key.identity_options().await.expect("looks").is_empty(),
            "no key, so ssh's own"
        );

        let other = key.clone();
        let (first, second) = tokio::join!(key.public_key(), other.public_key());
        let public = first.expect("makes it");
        assert_eq!(second.expect("the same"), public);
        let text = std::fs::read_to_string(&public).expect("reads it");
        assert!(text.starts_with("ssh-ed25519 "), "{text}");
        assert!(text.trim_end().ends_with(" agentZ"), "{text}");
        let mode = std::fs::metadata(key.path())
            .expect("private half")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "only its owner reads the private half");
        let names: Vec<String> = std::fs::read_dir(folder.path().join("cloud"))
            .expect("lists")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");

        std::fs::remove_file(&public).expect("removes the public half");
        assert_eq!(key.public_key().await.expect("derives it"), public);
        assert_eq!(
            std::fs::read_to_string(&public)
                .expect("reads it")
                .split(' ')
                .nth(1),
            text.split(' ').nth(1)
        );

        let options = key.identity_options().await.expect("looks");
        assert_eq!(
            options.first(),
            Some(&("IdentityFile".to_string(), key.path().display().to_string()))
        );
        assert!(options.contains(&("IdentityFile".to_string(), "~/.ssh/id_ed25519".to_string())));
    }
}
