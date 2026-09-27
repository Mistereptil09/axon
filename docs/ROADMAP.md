# Axon roadmap

> Restart docs: [README](README.md) · [ARCHITECTURE](ARCHITECTURE.md) · [ROADMAP](ROADMAP.md) · [S0](S0.md)

## V1: a Jarvis-lite you'll actually use daily
**The V1 end-to-end check:**
1. From the chat bar, by voice in French: "summarize `notes.md` on the server and write it to `~/proj/summary.md`". The server is read, the write is `notify` with undo, and undo works.
2. "Run the tests": the exec barrier card shows the pending write, and one click runs `cargo test` inside the sandbox.
3. The sandboxed test can't read the daemon's keys.
4. A tampered or replayed approval is rejected.
5. From Telegram on the phone, you chat in the same conversation and see it live on the desk.
6. Saying "stop" or pressing the kill hotkey stops everything, even with the orchestrator down.
7. Killing the orchestrator mid-turn keeps every completed tool round, and the dangling call becomes `Interrupted`.
8. The morning briefing arrives on time. When the server daemon goes offline, a notice arrives.

Every slice: CI green (`fmt --check`, `clippy -D warnings`, `test`); you explain its data flow and its security property without looking at the code; you write the ADR.

| # | Slice | Objective | Done when |
|---|---|---|---|
| **[S0](S0.md)** | Workspace & threat model | Tooling you understand, and the rules written down | Repo on an internal disk (real permission bits); pinned toolchain; workspace (`shared`, `daemon`, `orchestrator`, `desk`, `cli`); CI green; `CLAUDE.md` rules; path resolution (config → systemd dir → XDG); one `Config`; secrets created `0600` + perms check; **ADR 0001 threat model** (the [trust model](ARCHITECTURE.md#4-trust-model) in your own words, with the [OPEN QUESTIONS](OPEN_QUESTIONS.md) named) |
| **S1** | Policy kernel | Own the security core | `check_path` / `check_command` / protected set / default deny list / never-root list, with unit tests + proptests; `Denial{code, message, detail}`; `RiskClass`, `Tier`, class→tier mapping; `ActionDescriptor` with a stable canonical hash; tests for old bugs #6 and #7 |
| **S2** | Daemon tools | Tools nobody can bypass | `read/list/search/write/edit/run` via capability handles; limits; partial output kept on timeout (#9); `axon-daemon tool …` works locally; the source-scan test passes |
| **S3** | Safety net | Undo + containment | Landlock sandbox for `run` (tests prove a child can't read the daemon state dir, can't reach the network unless `egress`, and can't connect to the session bus or the daemon socket: ADR for Q1); checkpoints with rewind + conflict refusal + gc; exec barrier logic; local Unix socket with **kill switch/pause** (the process tree is killed, new calls refused) |
| **S4** | Core runtime, offline | The event-log heart | SQLite rooted at `account_id` (every query takes one); durable vs live events; a writer task per conversation; schema version + upcast skeleton; scripted model + in-process tool host drive a turn; crash test → completed rounds kept, dangling → `Interrupted`; deterministic replay reproduces a turn |
| **S5** | Model provider + CLI | First real assistant turn | Streaming `openai_compat` provider + `Capabilities`; constrained tool calls; toolset preselection; typed results + evidence ledger; system prompt publishes tools, risk classes and tiers; `axon chat` with an in-process daemon; old bug #5 test; 5 golden turns recorded |
| **S6** | Link + pairing | Daemons on other machines | Ed25519 device keys; pairing token with the pinned public key (certificate renewal needs no re-pairing); WSS; `Hello` challenge-response; `CallTool` round trip; `call_id` at-most-once (`Started` recorded before running; a test crashes mid-call and gets `Interrupted`, not a second run); `CapabilitiesChanged` on policy reload (#1); reconnect; revoke; a turn uses a daemon on a second machine |
| **S7** | Approvals | Signed, fresh, no TOCTOU | Tiers from `policy.toml` + `grants.toml` (local edits, hot reload, expiry, audit); challenge with one nonce per item + expiry; `axon approve` signs; grouped cards across two daemons with one signature; single-use nonces; re-check before execution; `local` tier checked by key; taint raises tiers (ADR for Q3: what taints); tests: tampered, replayed, expired and swapped-target approvals are all rejected |
| **S8** | Desktop client | Daily loop without a terminal | `axon-desk`: tray state icons (ported marks); chat bar on a hotkey (KDE custom shortcut → `axon-desk toggle`); streaming answers + tool activity lines; inline approval + exec-barrier cards signed with the local key (the key is in the sandbox deny list); KDE notifications with Undo; kill-switch hotkey over the local socket; a `Notice` names which link is down |
| **S9** | Desktop tools + web | It feels like Jarvis | Typed D-Bus tools: media (MPRIS), volume, open app/URL, notify, clipboard, lock, system info (`desktop` class); `web_fetch` / search as a taint source with trust labels + defang |
| **S10** | Voice | Talk to it | STT/TTS provider traits (OpenAI-compat + local); push-to-talk; VAD; `voice.languages`; command-word list; TTS streamed per sentence with a fr/en voice; barge-in; mic muted while speaking; voice "yes" = `chat` tier; latency measured and written down |
| **S11** | Telegram | Chat from the phone | Adapter with long polling (no public port); allowlisted chat ids; same conversations as the desk; `chat`-tier inline buttons; `device` tier answers "approve at the desk"; ADR on the Telegram privacy trade-off |
| **S12** | Proactive basics | It speaks first, carefully | A scheduler that emits `Trigger` events; timers and reminders; morning briefing; health alerts (daemon offline, disk nearly full, provider down); attention budget (quiet hours, digest, rate limit); **report-only**; delivered to the desk when active, else Telegram |
| **S13** | Evals + injection posture | Measured, not assumed | Live-replay runner over the event log; golden turns scored; injection corpus scored on 3 outcomes on your daily model; the taint and exec-barrier paths exercised |
| **S14** | Daily use | Validate by living with it | orchestrator as a system service with a dedicated user (`DynamicUser=yes`, `StateDirectory=axon`); daemon and `axon-desk` as systemd user units; install doc (with recommended setup: Tailscale, dedicated host); 2–4 weeks of use; a friction log decides V2's order |

## V2: reach and memory (order decided by the S14 friction log)
- **Phone approvals:** a minimal approval page (one card, WebAuthn passkey, synced passkeys accepted), hosted on the orchestrator with the risk documented; Web Push or ntfy for pending approvals.
- **Remote grants:** "allow cargo in `~/proj` until 18:00" from the chat bar or phone, as a signed approval that the daemon writes into `grants.toml`.
- **Wake word** (openWakeWord), in addition to push-to-talk.
- **Presence routing:** screen lock and idle time decide between speaking at the desk and messaging Telegram.
- **Ambient sensors:** port the old daemon-first design (sensors, local rules, notify/escalate) onto `Trigger` events.
- **Memory** with provenance: only confirmed memories are saved, each with its source.
- **Files:** a content-addressed blob store; Telegram attachments into conversations; `upload_file` (egress) / `download_file` (hash-verified) daemon tools; text/image/PDF extractors with trust labels.
- **Telegram voice notes** through the STT provider.
- **Calendar and email, read-only** (email is a taint source).
- **OpenAI-compatible adapter** for stock frontends (includes conversation identification from resent history).
- **MCP bridge in the daemon** for local MCP servers (tools are `reversible() = false`, classified by policy).
- **Built-in rule packs** for common programs (git, cargo, npm, find…), maintained in code.
- **Explain-refusal** improvements and a policy capability diff shown when the policy changes.

## V3: hardening and breadth
- **End-to-end encryption between daemon and approvers.** The orchestrator relays bytes it can't read or change.
- **Split relay and brain:** only a tiny relay faces the network.
- **Split the daemon** into a guardian (system user, holds keys and decides) and an executor (you, holds nothing secret), so your own unsandboxed processes can't read the daemon's keys ([OPEN QUESTIONS Q4](OPEN_QUESTIONS.md#q4)).
- **Approval page on a separate static host,** so a compromised orchestrator can't change the approval UI.
- **Triage model** for uncertain ambient events. **Action-capable escalation** (always through approval tiers).
- More adapters: **Matrix / Signal** (private messaging), **ACP**, and **Axon as an MCP server**.
- A phonetic dictionary for mixed fr/en speech. A voice client on the phone.
- **Windows** daemon (AppContainer / restricted tokens, instead of Landlock).
- Multi-account features (the schema is ready).
