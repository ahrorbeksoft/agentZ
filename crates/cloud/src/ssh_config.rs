//! A `~/.ssh/config` Host block, as the CLIs' `ssh-config` print one, read into options for
//! `ssh -o`, so agentZ never edits the user's config.

use agentz_protocol::cloud::SshOptions;
use anyhow::{Context as _, Result};

pub(crate) fn parse(text: &str) -> Result<SshOptions> {
    let mut options = SshOptions::default();
    let mut host_name = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `Key Value` or `Key=Value`, as ssh_config(5) allows.
        let (key, value) = match line.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((key, value)) => (key, value.trim_start_matches([' ', '\t', '=']).trim()),
            None => continue,
        };
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or(value);
        match key.to_ascii_lowercase().as_str() {
            // The block's own name, which only `~/.ssh/config` uses.
            "host" => {}
            "hostname" => host_name = Some(value.to_string()),
            "port" => {
                options.port = Some(
                    value
                        .parse()
                        .with_context(|| format!("reading the port {value:?}"))?,
                )
            }
            "user" => options.user = Some(value.to_string()),
            _ => options.options.push((key.to_string(), value.to_string())),
        }
    }
    options.host_name = host_name.context("the Host block has no HostName")?;
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_protons_block() {
        let options = parse(include_str!("proton_outputs/ssh-config.txt")).expect("parses");
        assert_eq!(
            options,
            SshOptions {
                host_name: "ssh.proton.computer".into(),
                port: Some(22021),
                user: Some("ubuntu".into()),
                options: vec![
                    (
                        "UserKnownHostsFile".into(),
                        "/Users/alex/.config/proton/known_hosts".into()
                    ),
                    ("StrictHostKeyChecking".into(), "yes".into()),
                ],
            }
        );
    }

    #[test]
    fn reads_equals_signs_and_quotes() {
        let options = parse(
            "Host x\n  HostName=gateway.example\n  CertificateFile \"/tmp/with space-cert.pub\"\n",
        )
        .expect("parses");
        assert_eq!(options.host_name, "gateway.example");
        assert_eq!(
            options.options,
            [(
                "CertificateFile".to_string(),
                "/tmp/with space-cert.pub".to_string()
            )]
        );
    }

    #[test]
    fn a_block_without_a_host_name_fails() {
        assert!(parse("Host x\n  User ubuntu\n").is_err());
    }
}
