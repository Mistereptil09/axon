# 1. Open questions
Each one is decided in an ADR by the slice named, before that slice is done.

## Q1: Same-uid local sockets (decide in S3). The desk↔daemon socket and the session bus both live in `$XDG_RUNTIME_DIR`, and both accept any process running as you. A sandboxed `build.rs` could reach them: the daemon socket gives it pause/resume and status, and the session bus gives it `systemd-run --user`, a full escape.
- First, find out whether this kernel's Landlock (ABI 10 on the dev machine) can block connecting to a named Unix socket. Older ABIs couldn't.
- If it can't: a mount namespace that hides `$XDG_RUNTIME_DIR` from the child (the way bubblewrap does it). Unprivileged user namespaces work on the dev machine.
- Either way, S3 has a test: *a sandboxed child can't connect to the session bus or to the daemon socket.*
- Worth considering: pausing needs no signature (it only makes things safer), resuming does.

## Q2: btrfs snapshots without root (decide in S3). Creating a snapshot of `/home` as a regular user needs help (snapper with `ALLOW_USERS`, or similar), and that same permission most likely also lets the user *delete* snapshots (to verify). If so, "a process running as you can't delete it" doesn't hold. Verify, then pick:
- a tiny root helper that can only *create* snapshots, rate-limited (a narrow, documented exception to "never root");
- rely on snapper's own root-run timeline and drop the per-batch snapshot;
- keep per-batch snapshots but drop the claim that they survive a malicious process.
On the dev machine `/home` is btrfs (`@home`), but snapper only has a `root` config.

## Q3: What taints a conversation (decide in S7). §4 lists files as untrusted; §5 lists only web fetch, downloads and email as taint sources.
- If every file read taints, nearly every conversation ends up at `device` tier: approval fatigue.
- If no file read taints, a planted `notes.md` goes unnoticed.
- Middle ground to evaluate: only files that came from outside taint (downloaded, cloned, or carrying a "came from the internet" mark).

## Q4: Splitting the daemon (V3). Processes running as you that aren't sandboxed (a malicious npm package you installed yourself) can read the daemon's keys, grants and journal. The fix is two processes:

| Process | Runs as | Holds | Does |
|---|---|---|---|
| Guardian | system user `axon-guard` | keys, grants, journal | verifies approvals, decides every call |
| Executor | you | nothing secret | touches files, runs sandboxed commands, desktop tools |

The executor only accepts orders from the guardian (uid-checked socket). It's two processes, two sockets and a system plus a user service, so it's too much for V1. It's recorded in ADR 0001 as a known limitation.
