# Open questions

> Docs: [README](README.md) · [ARCHITECTURE](ARCHITECTURE.md) · [ROADMAP](ROADMAP.md) · [OPEN QUESTIONS](OPEN_QUESTIONS.md) · [S0](Vertical_slices/S0.md)

Each one is decided in an ADR by the slice named, before that slice is done.

## Q1: Same-uid local sockets
**Decide in S3.** The desk↔daemon socket and the session bus both live in `$XDG_RUNTIME_DIR`, and both accept any process running as you. A sandboxed `build.rs` could reach them: the daemon socket gives it pause/resume and status, and the session bus gives it `systemd-run --user`, a full escape.
- First, find out whether this kernel's Landlock (ABI 10 on the dev machine) can block connecting to a named Unix socket. Older ABIs couldn't.
- If it can't: a mount namespace that hides `$XDG_RUNTIME_DIR` from the child (the way bubblewrap does it). Unprivileged user namespaces work on the dev machine.
- Either way, S3 has a test: *a sandboxed child can't connect to the session bus or to the daemon socket.*
- Worth considering: pausing needs no signature (it only makes things safer), resuming does.

## Q2: btrfs snapshots
**Decided: dropped** ([ADR 01](decisions/01-threat-model.md) §5). Checkpoints (shadow git) are the only undo layer.

**Remaining gap, decide in S3:** checkpoints record only the writes made by the daemon's own tools. A `run` child can also write, and the sandbox allows it inside the workspaces (`cargo build`, a script, a formatter), but nothing records those changes, so they can't be undone. Writes outside the workspaces are already blocked by the sandbox.
- Candidate: commit each workspace to its shadow repo just before a `run` call (or batch), so a rewind covers what the command changed.
- Cost to check: commit time on a large workspace; files excluded by `.gitignore` (e.g. `target/`) stay uncovered.

## Q3: What taints a conversation
**Decide in S7.** [ARCHITECTURE §4](ARCHITECTURE.md#4-trust-model) lists files as untrusted; [§5](ARCHITECTURE.md#5-security-layers-on-each-daemon) lists only web fetch, downloads and email as taint sources.
- If every file read taints, nearly every conversation ends up at `device` tier: approval fatigue.
- If no file read taints, a planted `notes.md` goes unnoticed.
- Middle ground to evaluate: only files that came from outside taint (downloaded, cloned, or carrying a "came from the internet" mark).

## Q4: Splitting the daemon
**V3.** Processes running as you that aren't sandboxed (a malicious npm package you installed yourself) can read the daemon's keys, grants and journal. The fix is two processes:

| Process | Runs as | Holds | Does |
|---|---|---|---|
| Guardian | system user `axon-guard` | keys, grants, journal | verifies approvals, decides every call |
| Executor | you | nothing secret | touches files, runs sandboxed commands, desktop tools |

The executor only accepts orders from the guardian (uid-checked socket). It's two processes, two sockets and a system plus a user service, so it's too much for V1. It's recorded in [ADR 01](decisions/01-threat-model.md) as a known limitation.

## Q5: What an approved command really runs
**Decide in S3/S7.** An allowed binary can still run arbitrary code: `python -c`, `node -e`, `perl -e`, or a script file changed between approval and launch. Ideas from [harness research](research/01-harnesses.md):
- Inline-eval flags on interpreters always need approval, whatever the binary's tier (OpenClaw `strictInlineEval`).
- Hash script operands into the `ActionDescriptor` and refuse to run if the file changed since approval.

## Q6: Network egress for `run`
**Decide in S3.** Landlock restricts files, not the network, so a sandboxed child can still send data out.
- Option A: a seccomp filter that blocks network syscalls by default (Codex CLI).
- Option B: no direct network; go through a proxy outside the sandbox with a domain allowlist (Claude Code). Heavier, but it lets taint decide egress per domain.

## Q7: Home automation
**V2.** Control the house through a hub (Home Assistant), not a daemon on each device: a daemon next to the hub exposes typed tools (`set_cover`, `get_temperature`, `get_weather`).
- A new `home` risk class with per-entity tiers in `policy.toml`: curtains and lights `auto`; locks, heating, oven at least `device`, never from Telegram.
- The HA token has full access: it goes in the protected set, and the entity allowlist is the real permission system.
- Tools return only numbers, enums and known entity ids (no free text), so HA data doesn't need to taint.
- Presence data (occupancy, locks, cameras) counts as private data for taint and egress (Meta's Rule of Two).
- Wake-on-LAN for the model host is a typed daemon tool (`wake_pc`), so the orchestrator still never acts.

## Q8: Invisible characters
**Decide in S9.** Hidden zero-width Unicode in a calendar invite was enough to compromise goose. Defanging should strip or show invisible and bidi characters in every untrusted text before the model or the user sees it.
