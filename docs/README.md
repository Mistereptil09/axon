# Axon restart

> Restart docs: [README](README.md) · [ARCHITECTURE](ARCHITECTURE.md) · [ROADMAP](ROADMAP.md) · [S0](S0.md)

## What Axon is
A Jarvis-like assistant that acts on your computers on your behalf, and that you can trust not to break them.
- It is designed for **local models**, which are weaker and easier to manipulate than cloud models. So **the harness owns every guarantee**: talking to the model, giving it tools, and limiting what it can do.
- The model is treated as untrusted and not very capable. Safety comes from the harness's structure, never from the prompt.
- The harness also **shrinks each task** until the judgment left over fits a small model.

## The four pillars
1. **Safety:** it can't break the computer, and whatever it did can be undone.
2. **Action:** typed tools for common intents, and a sandboxed shell as the fallback.
3. **Presence:** a chat bar on a hotkey, voice, the tray, notifications, and Telegram when you're away.
4. **Proactivity:** timers, a briefing and health alerts first, ambient sensors later.

## Key decisions
| # | Decision | Reason |
|---|---|---|
| 1 | The core is an **append-only event log** per conversation, with **one writer task per conversation** | History, sync across clients, audit and replay all come from one mechanism. |
| 2 | **Daemons dial out** to the orchestrator and never listen on the network | No inbound port on your machines. The orchestrator never dials a URL it was given.|
| 3 | **The daemon enforces everything.** The orchestrator is a relay for approvals | A hijacked orchestrator can delay or drop an approval (timeout = deny) but can't forge one. |
| 4 | **Signed approvals** (Ed25519) from **local, installed** approvers: the desktop chat bar and the CLI | The approver's code isn't served by the orchestrator, so what you see is what you sign. |
| 5 | **No PWA in V1.** The phone gets chat through **Telegram**, with `chat`-tier buttons | Much less work. Risky approvals happen at the desk. A phone approval page is V2. |
| 6 | Risky actions have a **safety net: checkpoints (shadow git) + Landlock sandbox** | Undo instead of asking. The sandbox keeps approved code away from the daemon's keys and grants. |
| 7 | **The exec barrier:** writes are silent (`notify`), but before any command runs, unreviewed writes must be kept or rolled back | Closes the "write `build.rs`, then run `cargo`" hole without a new risk class. |
| 8 | **Taint:** once untrusted content enters a conversation, egress and exec go up a tier | Defense against the "lethal trifecta" (private data + untrusted input + a way out). It's deterministic and needs no model judgment. |
| 9 | **Never root:** `sudo`/`pkexec`/`su`/`doas` are refused, and the daemon runs as the logged-in user | The system can't be broken. Only user data is at risk. The daemon acts on *your* files, session D-Bus and tools, which a separate user can't reach. |
| 10 | Everything outside the core sits **behind a trait**: model, STT and TTS providers (OpenAI-compatible first), and client adapters | Orchestrator, model host, daemon and client can each be on a different machine. |
| 11 | **Typed tools** | `set_volume(40)` is safe, needs no approval, and a small model gets it right. |
| 12 | **Kill switch local to the daemon** | "Stop" must work when the orchestrator or the network is down. |
| 13 | **Single user in V1**, with `account_id` kept in the schema | Multi-account features cost a lot and help nobody yet. |
| 14 | **Errors:** a stable reason code + a friendly message + the raw detail, all visible to the model | An error may show anything the environment description already shows. |
| 15 | **Cancel ≠ rollback:** cancel stops what is running, rollback undoes effects. `Tool::reversible()` defaults to `false` | Not every action can be undone (commands, MCP tools), and the UI says so. |
| 16 | The **orchestrator runs as a dedicated system user** (`DynamicUser=yes`) in production | It never touches your files, so a hijacked orchestrator can't read your home. The daemon can't do the same (see #9); splitting it is [Q4](OPEN_QUESTIONS.md#q4). |
| 17 | Devices **pin the orchestrator's public key** | The certificate can be renewed without pairing every device again. |
| 18 | Tool calls run **at most once**: the daemon records `Started` before running | Re-running a command after a crash could do it twice. A call that started but never finished is `Interrupted`. |
| 19 | The `local` tier is defined **by the key** (this machine's `axon-desk` key), not by the socket it arrives on | The local socket only checks the uid, which any process of yours, sandboxed children included, also passes. |

Still open: [OPEN QUESTIONS](OPEN_QUESTIONS.md) (same-uid sockets, snapshots without root, taint scope, daemon split).

## What carries over from the old code
- Enforcement in the daemon, fail closed, policy edited by humans only, and the protected set.
- **The path guard:** canonicalize the nearest ancestor, and decide every refusal before checking whether the file exists.
- **The command guard:** no shell, absolute program resolution, refuse binaries inside roots, and rebuild the child's environment.
- Capability handles plus the source-scan test.
- Provenance labels and the reserved marker namespace.
- Replayable evals.
- Worth porting once understood:
  - `guard.rs`: `anchor_of`, `Protected`, `check_program_location`
  - `caps.rs`: `drain_tail`, `build_command`, `spawn_grouped`, bounded read
  - `shared/spawn_env.rs`
  - the defanging idea in `trust.rs`
  - the eval fixtures
  - the ambient daemon-first design (`features/ambient.md`)
  - the tray icon states (`assets/tray/mark-*.svg`)

## Cut, or deferred indefinitely
- OpenAI-first facade: a V2 adapter.
- Chain-hash threading: it only comes back inside the OpenAI adapter.
- Dial-back MCP.
- Repair ladders, condensation (replaced by the evidence ledger), best-of-N sampling, the critic pass.
- GUI "computer use" (screenshots plus clicks): Wayland blocks it and small models are unreliable at it.
- WebAuthn in V1, multi-account features, per-argument rule packs written by users.
- ACP, Matrix, Windows (V3).

## Code rules
- Errors are `thiserror` enums per layer, and text is rendered only at the edge.
- One `Config` struct per binary. Env is used only for secrets and for paths (`XDG_*`, and the directories systemd hands a service: `STATE_DIRECTORY`, `RUNTIME_DIRECTORY`, `CONFIGURATION_DIRECTORY`).
- Paths resolve in one order: explicit config path → systemd directory → XDG default. Nothing is hardcoded.
- Comments state invariants in 1–2 lines. History goes in ADRs.
- Runtime state lives in XDG dirs on a real filesystem (not exFAT: old bug #3). Secrets are created `0600` in one step (never create-then-chmod), and loading one that is group- or world-readable is an error.
- The repo lives on an internal disk: exFAT has no permission bits, so the secrets tests can't pass there.
- Tests use `tempfile` and leave no artifacts in the repo (old bug #10).

## Old findings and where they're fixed
| Old bug | Fixed in |
|---|---|
| #1 environment not refreshed after a policy reload | S6 `CapabilitiesChanged` |
| #2 unvalidated `mcp_url` dialed | S6, the daemon dials out |
| #3 secrets on exFAT | S0 XDG + perms check |
| #4 failed turns record nothing | S4 event log |
| #5 `tool_call_id` ignored | S5 test |
| #6 denied-glob refusal depends on existence | S1 |
| #7 relative paths resolved against the daemon's cwd | S1 |
| #8 spaces embedded in a string literal (`caps.rs:515`) | S2 (use `\` line continuations or `concat!`) |
| #9 timeout discards partial output | S2 |
| #10 test artifacts left in the repo | S0 |
