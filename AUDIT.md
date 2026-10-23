# agentZ project audit

Audited on October 23, 2026, against commit `c0194cbe8216c0619ebc089c3e833abc6f21f505` (v0.4.2), on macOS arm64 with Rust 1.98.1.

**Seven actionable findings: five P1 and two P2. Six have executable reproductions against the current implementation; the seventh is a published security advisory affecting shipped TLS dependencies. Three findings can destroy user data.** P1 means fix before the next release; P2 means schedule a fix soon. These are project priorities, not CVSS scores.

The shared project, build directory, running app, servers, and other agent's edits were left untouched. All compilation and reproductions ran in `/tmp/agentz-audit-5bqggwzs`, using temporary data and repositories. No prompts were sent to real agents. The final comparison of 1,888 tracked files found only the other agent's two design files had changed since the snapshot: `design/model-visibility/choices.json` and `design/model-visibility/decisions.md`. All finding locations still matched the shared source at delivery.

## Findings

| ID | Priority | Problem | Evidence |
| --- | --- | --- | --- |
| A1 | P1 | Unreadable state triggers deletion of surviving conversations and attachments | Real scratch server deleted both files |
| A2 | P1 | Artifact filename injects HTTP headers and removes the response sandbox | Raw HTTP response from real scratch server |
| A3 | P1 | Pasture cleanup follows symlinks and deletes files outside the clone | External file deleted during pasture creation |
| A4 | P1 | Pasture removal ignores unique commits on other branches | `remove(..., false)` deleted the only repository holding the commit |
| A5 | P1 | Shipped TLS verifier and fallback trust roots have published security fixes | RustSec scan, dependency paths, TLS configuration review |
| A6 | P2 | Overlapping settings saves can overwrite or corrupt a newer save | Deterministic race against the actual write primitive |
| A7 | P2 | Saving credential JSON widens private file permissions | An existing `0600` file became `0644` |

### A1. Stop destructive cleanup when loading state fails

Locations: [ProjectStore::load](/Users/ahrorbek/projects/agentZ/crates/projects/src/projects.rs:978), [startup cleanup](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/server.rs:568).

`ProjectStore::load` logs a read or deserialization error and substitutes an empty store. Server startup then treats every existing thread transcript, attachment directory, and continuation as an orphan and deletes it. A corrupt `state.json`, or a non-NotFound read failure, can therefore destroy the material needed to recover the user's conversations. This happens without the user requesting deletion.

Reproduction `audit_corrupt_state_must_preserve_conversations_and_attachments` creates invalid state JSON alongside a transcript and attachment, starts the real server with that scratch data, and shuts it down. Both files were gone: `transcript=false, attachment=false`. Continuation cleanup has the same dependency on the empty store; that file type was inspected but not included in the reproduction.

Fix: distinguish a missing state file from a failed load. Propagate the failure or enter a recovery state that preserves all existing files. Only perform orphan deletion after successfully loading authoritative state. Keep the unreadable index available for recovery.

### A2. Reject control characters before constructing artifact response headers

Locations: [filename validation](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/artifacts.rs:1168), [Content-Disposition](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/pages.rs:520), [HTTP serialization](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/pages.rs:834).

Artifact filenames accept embedded carriage returns and newlines. The file handler interpolates the filename into `Content-Disposition`, escaping only quotation marks, before appending its sandbox CSP. A filename containing `\r\n\r\n` ends the HTTP headers early: the intended CSP becomes response body text and attacker-chosen HTML can precede the stored file bytes.

Reproduction `audit_artifact_filename_must_not_remove_http_sandbox` publishes a document attachment named `report\r\n\r\n<svg onload=alert(1)>\r\npreview.html` and requests it from the real server using the artifact's frame key. It returned `200 OK` with HTML content type. The actual header section ended at `content-disposition: inline; filename="report`; the body began with the SVG markup followed by the displaced CSP header.

The raw response sandbox bypass is proven. Direct navigation or opening the attachment in a new tab could consequently execute supplied HTML at the pages server's origin; that browser behavior is an inference, not a browser exploit test. Existing iframe sandbox attributes still constrain frames, so this finding does not assert an iframe sandbox escape. The route needs the artifact's frame key, not the privileged pages API token.

Fix: reject control characters in filenames at publication and request boundaries. Validate HTTP header values and correctly encode `Content-Disposition`; do not concatenate unchecked values into raw HTTP. Retain a regression that verifies the CSP remains in the header section.

### A3. Keep post-clone deletion inside the pasture

Locations: [cleanup rules](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/workspaces.rs:412), [glob traversal](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/workspaces.rs:439).

`expand_pattern` rejects `..` but follows intermediate directory symlinks. For `linked/*`, it first accepts `linked` using `symlink_metadata`, then calls `read_dir` on it, which follows the link. Checking only the final result with `symlink_metadata` does not protect its ancestors. Cloning preserves symlinks, including absolute links back to the source project or elsewhere.

Reproduction `audit_clone_cleanup_must_not_delete_through_an_external_symlink` creates a repository with an absolute `linked` symlink to an external directory and `.cow.json` containing `{"post_clone":{"remove":["linked/*"]}}`. Creating a pasture deleted `outside/precious.txt` outside the clone.

Fix: refuse to descend through symlinks in intermediate pattern components, and verify deletion targets remain inside the pasture. Use traversal that does not re-follow symlinks between validation and deletion. This finding concerns cleanup patterns; explicitly configured `post_clone.run` commands are intentionally arbitrary commands.

### A4. Preserve commits reachable from every local reference before removing a pasture

Location: [pasture removal guard](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/workspaces.rs:502).

With `force=false`, pasture removal checks the working tree and whether the current `HEAD` object exists in the source project. It never inspects other local branches. A clean pasture switched back to its initial branch passes both checks even if another branch contains the only copy of new commits. The subsequent directory removal deletes that branch and its object database without requesting confirmation.

Reproduction `audit_pasture_removal_must_protect_commits_on_other_branches` commits work on `valuable-work`, verifies that the source project does not contain its SHA, switches back to `audit-work`, and calls removal without force. The result was `Removed` rather than `NeedsConfirmation`.

Fix: check unique work reachable from all local branches and other refs that can contain user work, including stashes. Preserve or import those refs before deleting the clone, or require explicit confirmation. The related [unpushed-commit helper](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/workspaces.rs:994) also counts only `HEAD`; automatic cleanup should be audited against the same invariant. Automatic cleanup and stash loss were not independently reproduced.

### A5. Update the shipped TLS verifier and fallback trust anchors

Locations: [rustls lock entry](/Users/ahrorbek/projects/agentZ/Cargo.lock:6886), [webpki lock entry](/Users/ahrorbek/projects/agentZ/Cargo.lock:6960), [root bundle lock entry](/Users/ahrorbek/projects/agentZ/Cargo.lock:9248), [TLS fallback](/Users/ahrorbek/projects/agentZ/crates/http_client_tls/src/http_client_tls.rs:18).

The app and server both depend on `reqwest_client → http_client_tls → rustls 0.23.45 / rustls-webpki 0.103.15 / webpki-roots 1.0.8`. These are present in the normal macOS and Linux dependency graphs.

RustSec reports that the pinned verifier and bundled roots can accept paths through distrusted emSign intermediates whose private keys are public, subject to the configured trust anchors. The TLS configuration explicitly falls back to the bundled roots if platform-verifier initialization fails. This makes the vulnerable fallback concrete; it does not establish that every normal connection using the operating system verifier is affected. Update `rustls-webpki` to at least `0.103.16` and `webpki-roots` to at least `1.0.10`. Sources: [verifier advisory](https://rustsec.org/advisories/RUSTSEC-2026-0382.html), [root-bundle advisory](https://rustsec.org/advisories/RUSTSEC-2026-0381.html).

Also update `rustls` to at least `0.23.46`. One applicable configuration concern is cached key-share prediction allowing a network attacker to reduce later connections' post-quantum protection, depending on server policy. This does not bypass ordinary authentication. [Rustls advisory](https://rustsec.org/advisories/RUSTSEC-2026-0349.html).

Evidence is the published advisories, actual resolved versions, and inspected TLS configuration. No forged-certificate connection or network interception was attempted. Resolve compatible patched versions in the lockfile, rerun platform checks, and add an advisory check to CI.

### A6. Serialize settings saves

Locations: [background save scheduling](/Users/ahrorbek/projects/agentZ/crates/app/src/app_settings.rs:501), [shared settings temporary path](/Users/ahrorbek/projects/agentZ/crates/app/src/app_settings.rs:565), [file replacement primitive](/Users/ahrorbek/projects/agentZ/crates/projects/src/projects.rs:2636).

Each settings change starts a background task writing the same `settings.json.tmp`. Replacing the stored task handle cannot interrupt a synchronous filesystem operation already executing on another worker. Two tasks can therefore open the same temporary inode. If the newer task writes and renames it first, the older task's open handle now refers to the final settings file and can overwrite it after the successful rename. The older rename then fails, after damaging the newer settings. Different payload lengths can also leave mixed bytes.

Reproduction `audit_overlapping_settings_saves_must_not_overwrite_a_newer_save` deterministically orders these operations using the real `projects::write_durably`. The final file contained `older!` rather than `newest`. This proves the filesystem interleaving; the complete GPUI settings interaction was not driven to reproduce the timing.

Fix: use one ordered saver that coalesces updates and writes them serially, as the project store does. Unique temporary filenames alone prevent shared-inode corruption but do not guarantee the newest settings win.

### A7. Preserve restrictive permissions when saving credentials

Locations: [JSON save helper](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/agent_settings.rs:98), [MCP configuration persistence](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/mcp_servers.rs:28), [temporary-file creation](/Users/ahrorbek/projects/agentZ/crates/projects/src/projects.rs:2641).

The shared JSON writer creates a new temporary file with default permissions and renames it over the old file. It does not preserve the old file's restrictive mode or request owner-only access. Under umask `022`, even an existing `0600` credentials file becomes `0644`. Agent settings and MCP configuration can contain API keys in environment variables or HTTP headers.

Reproduction `audit_secret_json_must_keep_owner_only_permissions` saves through the real JSON helper over an existing `0600` file. Its resulting mode was `0644`. Other OS users can read such credentials where parent directories are traversable; a private home or data directory can mitigate exposure. The scratch reproduction's private parent directory prevented exposure to other users during testing. The separate account API-key storage has explicit restrictive handling and is not implicated by this finding.

Fix: create secret-bearing files as `0600`, preserve an existing more restrictive mode where appropriate, and ensure containing data directories have the intended permissions. Set the mode before writing secrets, including on the temporary file.

## Dependency scan details

`cargo-audit 0.22.2` checked 997 lockfile dependencies against RustSec database commit `f7b2f6bc663452200bbe639ebb75e0c762b3422b`, updated October 23, 2026. It returned 17 vulnerability matches representing 13 distinct advisory IDs, plus 10 unmaintained, 2 unsound, and 4 yanked-package warnings. A match is not proof an exploitable code path is used.

| Package and locked version | Advisory matches | Triage |
| --- | --- | --- |
| `rustls-webpki 0.103.15`, `webpki-roots 1.0.8` | 0382, 0381 | Shipped on both platforms; conditional certificate trust issue in A5 |
| `rustls 0.23.45` | 0346, 0347, 0348, 0349 | Shipped on both platforms; update to 0.23.46+; no custom session-debug or ECH configuration found, and Brotli feature is absent from the inspected macOS graph |
| `crossbeam-epoch 0.9.18` | 0204 | Shipped on both platforms; affected pointer formatting was not demonstrated; patched in 0.9.20+ |
| `quick-xml 0.37.5`, `0.39.3` | 0194 and 0195 for each version | Linux Wayland/protocol and accessibility-generation paths; untrusted runtime XML reachability was not established |
| `quick-xml 0.30.0` | 0194, 0195 | Not in the inspected normal app/server graphs |
| `quinn-proto 0.11.17` | 0343, 0338, 0340 | Not in the inspected normal app/server graphs |
| `webpki-root-certs 1.0.9` | 0383 | Not in the inspected normal app/server graphs |

All IDs in this table have prefix `RUSTSEC-2026-`. The quick-XML advisories concern parsing CPU and allocation limits; they do not by themselves show that agentZ parses attacker-controlled XML on these paths. [Upstream duplicate-attribute advisory](https://rustsec.org/advisories/RUSTSEC-2026-0194.html). The rustls Brotli issue requires that optional feature. [Upstream Brotli advisory](https://rustsec.org/advisories/RUSTSEC-2026-0348.html). The crossbeam issue is specific to affected formatting operations. [Upstream crossbeam advisory](https://rustsec.org/advisories/RUSTSEC-2026-0204.html).

The unsound warnings concern `event-listener 5.4.1` (fixed in 5.4.2+) and `memmap2 0.9.8` (fixed in 0.9.11+). Both occur in the macOS graph; their triggering operations were not reproduced. Yanked entries are `chacha20 0.10.1`, `core2 0.4.0`, `spin 0.9.8`, and `spin 0.10.0`. The JSON report retains every advisory, description, affected range, and maintenance warning.

## Other review observations

These are code-supported follow-ups, not additional reproduced defects.

- **Client backpressure:** [connection.rs](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/connection.rs:40) uses an unbounded outgoing queue and awaits socket writes without a write deadline. A slow connection can retain queued events while producers keep running. Measure stalled SSH/local clients under terminal and transcript traffic; bound queued bytes or disconnect and resnapshot. Sustained memory growth was not load-tested.
- **Server responsiveness:** [save_transcript](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/server.rs:2165) performs full JSON serialization and durable synchronous file writes on the server actor. [Transcript persistence](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/transcripts.rs:18) confirms the file and directory synchronization path. Measure tail latency with large histories and many active threads; move ordered persistence off the actor if needed. No latency benchmark was run.
- **Automation:** the checked-in workflows cover site publishing and releases. [Release publishing](/Users/ahrorbek/projects/agentZ/.github/workflows/release.yml:130) follows platform builds without a checked-in test, Clippy, formatting, or advisory gate. Add a pull-request check workflow and make releases depend on it. Repository or organization settings outside these files were not inspected.
- **One flaky existing test:** an earlier baseline run observed `stops_the_agent_of_an_archived_thread_left_idle` reading an empty close-log file. It [waits for existence](/Users/ahrorbek/projects/agentZ/crates/agentz_server/src/tests.rs:1129), then immediately asserts its line count; file creation can precede the mock's completed write. The final complete run passed. Wait for complete contents rather than existence to make this assertion reliable.

## Scope and verification

The review covered the architecture and principal application paths across the app, agent thread/ACP v1-v2 handling, server/client protocol and reconnection, persistence, workspace creation/removal and Git operations, registry and installer, accounts/usage and MCP, terminal/child-process lifecycle, browser/artifact HTTP boundaries, cloud/SSH integration, settings, release tooling, and copied framework changes. This was a broad project audit with targeted deep investigation, not a claim that every line or external dependency was exhaustively verified.

The own-crate inventory is approximately 205,000 source lines. Of 481 compared vendored Rust files, 444 matched the local Zed reference byte-for-byte, 32 differed, and 5 were new. Comparison and focused reading concentrated on modified GPUI scrolling, rendering, window/platform behavior, and app integrations. Unchanged upstream code was sampled rather than exhaustively audited.

| Check | Result |
| --- | --- |
| Existing recommended Rust tests | **941 passed, 0 failed, 2 ignored** across 15 test targets |
| Recommended selected-crate Clippy, all targets | Passed |
| `cargo fmt --all -- --check` on the unmodified snapshot | Passed |
| GPUI scroll behavior tests | 3 passed |
| Metal drawable-provider tests | 2 passed |
| Six added audit regressions | **6 failed as expected**, proving A1–A4, A6, A7 |
| Dependency advisory scan | Failed with 17 matches; triaged above |

The ignored tests are `connects_to_a_real_machine` and `node_runtime::tests::managed_node_downloads_and_runs`. The snapshot needed an empty Git repository on branch `audit-snapshot` for the existing repository-inspection test; this was an audit fixture, not a source modification. GPUI tests used runtime shader features because this Mac lacks the full Xcode Metal compiler. Test builds disabled incremental compilation and debug symbols to limit disk usage. `block 0.1.6` generated a future-incompatibility warning.

No Linux build or runtime, live SSH/cloud operation, UI screenshot/manual interaction, release packaging/signing, real vendor-agent turn, browser exploit execution, or high-volume soak test was performed. Those are the material remaining coverage limits. No fixes were applied.

## Reproduction and artifacts

The supporting evidence is retained in `audit-evidence/2026-10-23-c0194cb/`:

- [Regression source](audit-evidence/2026-10-23-c0194cb/audit_regressions.rs) and [failure output](audit-evidence/2026-10-23-c0194cb/regressions.log).
- [Full existing test output](audit-evidence/2026-10-23-c0194cb/tests.log), [Clippy output](audit-evidence/2026-10-23-c0194cb/clippy.log), [format check output](audit-evidence/2026-10-23-c0194cb/fmt.log), [scroll tests](audit-evidence/2026-10-23-c0194cb/gpui-scroll-tests.log), and [Metal tests](audit-evidence/2026-10-23-c0194cb/metal-tests.log).
- [Advisory JSON](audit-evidence/2026-10-23-c0194cb/dependencies.json), [macOS dependency graph](audit-evidence/2026-10-23-c0194cb/dependency-tree-macos.txt), [Linux dependency graph](audit-evidence/2026-10-23-c0194cb/dependency-tree-linux.txt), and [rustls features](audit-evidence/2026-10-23-c0194cb/rustls-features.txt).
- [SHA-256 manifest of audited tracked files](audit-evidence/2026-10-23-c0194cb/source-sha256.json).

The temporary source snapshot `/tmp/agentz-audit-5bqggwzs` and its temporary logs were removed at the user’s request after the audit. The report, regression source, evidence logs, dependency scan, and source manifest remain in this repository with the report. The shared project and its target directory were not touched. To reproduce against a fresh isolated copy of the audited source, copy `audit_regressions.rs` into `crates/agentz_server/tests/` and run:

```sh
umask 022
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --locked --offline -p agentz_server --test audit_regressions audit_ -- --test-threads=1
```

These regressions intentionally fail until the defects are fixed; they touch only temporary test data. For the baseline recommended suite, remove that added integration-test file from the isolated copy first:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --locked --offline --no-fail-fast \
  -p app -p agent_thread -p projects -p registry -p agentz_protocol \
  -p agentz_server -p agentz_client -p cloud -p child_process \
  --features gpui_platform/runtime_shaders
```

Suggested fix order: stop destructive state cleanup; contain pasture deletions and preserve all refs; close artifact header injection; update the TLS dependencies; serialize settings persistence and preserve credential permissions. Add each reproduction as a permanent regression alongside its fix.
