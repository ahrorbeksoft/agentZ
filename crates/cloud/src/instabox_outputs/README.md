Captured from `instabox` (version `cbecf34a`) on 2026-10-18, with the account's details
replaced: `ls --json`, `get --json`, `templates --json`, `usage --json`, `whoami --json`,
`vault show --json`, `port ls --json` and `ssh-config`. `snapshots.json` isn't captured: the
account had no snapshots (`snapshot ls --json` printed `[]`), so it follows the snapshot objects
of Instabox's SDK (`@insforge/instabox-sdk` 0.5.3) *(unverified)*. The account had no published
ports or Vault items either, so their fields follow the SDK and the CLI's help *(unverified)*.
