# Axon architecture

> Docs: [README](README.md) · [ARCHITECTURE](ARCHITECTURE.md) · [ROADMAP](ROADMAP.md) · [OPEN QUESTIONS](OPEN_QUESTIONS.md) · [S0](Vertical_slices/S0.md)

## 1. Components

| Component | Binary | Runs on | Role | Listens on the network? |
|---|---|---|---|---|
| **Orchestrator** | `axon-orchestrator` | any machine (home server, desktop) | Core: conversations, event log, agent loop, approval relay, device registry, scheduler; adapters (client WS, Telegram) | **Yes, one port** (plain WebSocket; no TLS in V1, see [§3](#3-connections)) |
| **Daemon** | `axon-daemon` | each machine Axon acts on | Tools, policy, guard, sandbox, checkpoints, grants, verifies approvals, kill switch | **No.** A local Unix socket only. |
| **Desktop client** | `axon-desk` | the machine you sit at | Tray icon (state), **chat bar** (hotkey), **voice** (push-to-talk, STT/TTS via providers), notifications, **local approver** (holds an approver key) | No |
| **CLI** | `axon` | anywhere | `axon chat`, `axon approve`, `axon pair`, admin | No |
| **Model provider** | external (llama.cpp, Ollama, vLLM, cloud) | any machine (a GPU box) | Chat completions, OpenAI-compatible | its own HTTP port |
| **STT / TTS providers** | external (whisper.cpp server, Piper/Kokoro server) or in-process | the desk machine (recommended) or a GPU box | `/v1/audio/transcriptions`, `/v1/audio/speech` | their own HTTP port |
| **Telegram** | Telegram's servers | — | Phone chat, `chat`-tier buttons | — (the orchestrator polls it) |

The tray and the chat bar are **one process** (`axon-desk`) to keep the component count down.

**Which user each runs as:**
- **Orchestrator:** a dedicated system user in production (systemd `DynamicUser=yes`, `StateDirectory=axon-orchestrator`, `ProtectHome=yes`). It never needs your files. During development it runs as you.
- **Daemon and `axon-desk`:** the logged-in user, as systemd user units started at login. They act on your files, your session D-Bus and your tools (`~/.cargo`, git config), none of which another user can reach.
- Paths therefore resolve as: explicit path (in the config file; `--config` for the config file itself) → systemd directory variable (`STATE_DIRECTORY`…) → XDG default, so the same binary works in both setups. The XDG default has one subfolder per binary (`~/.local/state/axon/daemon`, `…/axon/desk`), so the daemon's and the desk's files never mix; systemd units name one directory per service.

## 2. Network map

```
                                   ┌───────────────────────────┐
                                   │   Model provider (GPU box)│
                                   │   OpenAI-compat HTTP(S)   │
                                   └─────────────▲─────────────┘
                                                 │ ③ HTTPS, API key (outbound from orchestrator)
 ┌───────────────── desk machine ──────────┐     │
 │ axon-desk (tray · chat bar · voice)     │  ②  │           ④ HTTPS long polling (outbound)
 │   │ STT/TTS providers (local or remote) ├──WS───▶ ┌──────────────────────┐ ─────────▶ Telegram API
 │   │ ⑥ Unix socket (kill switch,         │         │   ORCHESTRATOR       │
 │   ▼    local-tier approvals)            │         │   one port (no TLS)  │
 │ axon-daemon ────────────────────────────┼──WS───▶ │   /link  (daemons)   │
 │ guard · sandbox · checkpoints · journal │   ①     │   /client (clients)  │
 └─────────────────────────────────────────┘         └──────────▲───────────┘
                                                                │ ① WS
 ┌──── headless server ────┐                                    │
 │ axon-daemon  ───────────┼────────────────────────────────────┘
 └─────────────────────────┘
 anywhere: axon CLI ──② WS──▶ orchestrator
```

**Every arrow points toward the orchestrator or outward.** The only listening socket on the network is the orchestrator's. Daemons and clients have no inbound ports.

## 3. Connections

| # | From → To | Transport | Authentication | Carries |
|---|---|---|---|---|
| ① | daemon → orchestrator `/link` | Plain WebSocket in V1 (**no TLS**: trusted network only, see [ADR 01](decisions/01-threat-model.md)). V2: TLS, **orchestrator public key pinned** at pairing | Device **Ed25519** key, challenge-response in `Hello` (no bearer token) | `CallTool`, `ToolResult`, `ApprovalChallenge`/`Response`, `CancelCall`, `CapabilitiesChanged`, `Notify`, `Ping` |
| ② | client (desk, CLI) → orchestrator `/client` | Plain WebSocket in V1 (no TLS). V2: TLS, pinned | Device Ed25519 key, challenge-response | Commands: `SendMessage`, `Subscribe{conv, from_seq}`, `Cancel`, `Approve(signed)`, `Rollback`. Events stream back. |
| ③ | orchestrator → model / STT / TTS provider | HTTP(S), whatever the endpoint offers | API key from env/secret (optional for local) | OpenAI-compatible requests |
| ③' | desk → STT / TTS provider | HTTP(S) or in-process | API key if remote | audio in, text out / text in, audio out |
| ④ | orchestrator → Telegram API | HTTPS long polling (outbound only) | Bot token (secret); **only allowlisted chat ids** are accepted | messages, inline-button callbacks |
| ⑥ | desk → local daemon | Unix socket in `$XDG_RUNTIME_DIR/axon/daemon/`, mode 0600 | `SO_PEERCRED`: same uid only. **Not enough on its own:** every process of yours passes it, sandboxed children included ([Q1](OPEN_QUESTIONS.md#q1-same-uid-local-sockets)) | kill switch / pause, `local`-tier approvals (signed, so the uid check isn't what protects them), status |

**No TLS in V1: ① and ② are plain WebSocket, so they must only run on a trusted network** (localhost, a LAN you control, or a VPN such as Tailscale, which then provides the encryption). On an untrusted network, anyone on the path can read every conversation and inject messages after `Hello`. Signed `device`/`local` approvals still can't be forged or replayed; everything else can be. See [ADR 01](decisions/01-threat-model.md) §2b and §4. Adding TLS with a pinned key (V2) is what lifts this restriction; after that, a VPN becomes optional.

**Once TLS is added, pin the key, not the certificate:** devices store a hash of the orchestrator's public key (SPKI), so the certificate can be renewed with the same key without pairing again.

### Pairing
1. On the orchestrator: `axon admin pair-code --roles tool_host` prints a **pairing token** = host + one-time code + **the orchestrator's public-key fingerprint** (only used once TLS exists).
2. On the device: `axon pair <token>`. The device generates its Ed25519 keypair locally, connects (with the key pinned once TLS exists; no trust-on-first-use), proves it holds the code, and enrolls its public key and roles.
   - In V1 the one-time code travels in clear, so pair only on the trusted network.
3. Roles: `tool_host` (daemon), `approver`, `client`. One device can hold several.
4. **Trusting an approver key on a daemon** is local only:
   - `axon-daemon trust-approver <fingerprint>` on that machine.
   - The local `axon-desk` key is added directly.
   - A remote approver's fingerprint is compared by eye (shown on both screens).
   - It never comes from the orchestrator.
5. Revocation: `axon admin revoke <device>` on the orchestrator, and `axon-daemon untrust-approver` on daemons.

## 4. Trust model

| Party | Trusted for integrity (actions) | Trusted for confidentiality (data) |
|---|---|---|
| You at the desk (`axon-desk`, CLI) | yes, it signs approvals | yes |
| Daemon | yes, it is the enforcement point | yes (its own machine) |
| Orchestrator | **no** for `device`/`local` approvals (relay only); **yes** for `chat` tier | **yes**: it sees all conversations |
| Model | **never**: assumed injectable | it sees whatever it reads |
| Model provider host | — | sees everything sent to the model (**a user choice: local GPU box vs cloud**) |
| Telegram | `chat` tier only | **sees Telegram chats** (not end-to-end encrypted) |
| Tool output, web, files, email | never: labelled untrusted; which of these taint the conversation is [Q3](OPEN_QUESTIONS.md#q3-what-taints-a-conversation) | — |
| Sandboxed children (`run`) | never: they may be running code from an untrusted repo. They must reach neither keys nor local sockets | only the workspaces |
| Other processes running as you (not sandboxed) | not trusted, but **not defended against in V1**: they can read the daemon's keys. Known limitation, see [Q4](OPEN_QUESTIONS.md#q4-splitting-the-daemon) | same |

What each tier means under a compromised orchestrator:
- `chat`: the orchestrator can approve it.
- `device` / `local`: it can't.

## 5. Security layers on each daemon

**Workspace:** a folder listed in the daemon's `policy.toml` where Axon may act (e.g. `~/proj`, `~/notes`). Every workspace is checkpointed. A daemon tool refuses any path outside every workspace. (Called *roots* in older notes and ADR 01, `allowed_paths` in the old code.)

| Layer | What it does |
|---|---|
| **Never root** | Refuses `sudo`, `pkexec`, `su`, `doas`, setuid helpers. The daemon runs as the user, under a systemd user unit. |
| **Path guard** | The path must be inside a workspace. Absolute paths only (or one declared base). The nearest existing ancestor is canonicalized. Deny globs are decided before existence. The **protected set** covers `policy.toml`, `grants.toml`, keys, the journal, the daemon binary and its units. **Default deny list:** `.git/hooks/**`, `.git/config`, `.envrc`, shell rc files, autostart, systemd user units. |
| **Command guard** | No shell. The program is resolved to an absolute canonical path. Binaries inside workspaces are refused. The child env is rebuilt (`PATH`, no `LD_*`, `GIT_*`, `PAGER`…). The `exec_bounded` git forms run with `--no-pager`, `-c core.fsmonitor=false`, `-c core.hooksPath=/dev/null` (and `--no-ext-diff` for diff/log), because a repo's `.git/config` can make even `git status` run a program. |
| **Sandbox** | Every `run` child is under **Landlock**: read/write only in the workspaces plus system read-only paths, and **no access to the daemon's state/config dirs or the desk's (`axon/desk/`, which holds the approver key)**. **No network:** the child gets its own empty network namespace unless the call is classified `egress` (Landlock's network rules cover TCP only, so DNS over UDP would stay open). **No local sockets:** it must not reach `$XDG_RUNTIME_DIR`, which holds the session bus (`systemd-run --user` there = sandbox escape) and the daemon socket ⑥; how is [Q1](OPEN_QUESTIONS.md#q1-same-uid-local-sockets). Resources are capped (`systemd-run --user --scope`, `MemoryMax`, `TasksMax`), with a timeout and an output cap. |
| **Checkpoints** | A shadow git repo per workspace (separate `GIT_DIR`). Every write or delete by a daemon tool is committed under `refs/axon/conv/<conversation>` at its event `seq`. **Rewind** = restore files to their state before event N. Before restoring, the daemon checks that the file still has the recorded hash, and on a conflict it refuses and shows a diff. Deleting a conversation deletes its ref, and periodic `gc` removes commits nothing points to anymore. Retention is capped by age and size, and the workspace's `.gitignore` is respected. Writes made by a `run` child are not recorded yet: [Q2](OPEN_QUESTIONS.md#q2-btrfs-snapshots). |
| **Exec barrier** | A `run` call waits while any unreviewed write exists in the conversation. The card shows *command + pending writes*: [Keep all & run] / [Roll back & cancel] / per-item. A rollback returns `Cancelled(user_rollback)` to the model. |
| **Taint** | Tools that bring in outside content (web fetch, downloaded files, email later) set `ConversationTainted`. After that, `egress` and `exec_general` are raised one tier, for the rest of the conversation. Whether reading a local file taints is [Q3](OPEN_QUESTIONS.md#q3-what-taints-a-conversation). |
| **Limits** | Max tool calls per turn, repeated-call detection, per-tool timeouts, output caps. |
| **Kill switch** | A hotkey in `axon-desk` or the voice word "stop" goes to the local socket (⑥). The daemon kills every running tool's process group, enters **paused** (refusing `CallTool`), and tells the orchestrator best-effort. Resume is explicit. |

## 6. Risk classes and tiers

**Risk classes** are a closed enum in `shared/`. The daemon classifies from policy data, never from what the model claims.

| Class | Definition | Default tier |
|---|---|---|
| `read` | read, list or search inside workspaces | `auto` |
| `write_reversible` | a write or delete covered by checkpoints | `notify` (subject to the exec barrier) |
| `write_irreversible` | a write inside a workspace that checkpoints can't record: over the size cap, or a file the workspace's `.gitignore` excludes | `device` |
| `exec_bounded` | a **built-in** list of exact safe forms (`git status/diff/log`, `ls`…) | `auto` |
| `exec_general` | any other program (interpreters, build tools…) | `device` |
| `egress` | sends data off the machine (network program, upload, web fetch with data) | `device` |
| `observe_sensitive` | process details, clipboard read… | `device` |
| `desktop` | typed desktop tools (media, volume, open URL/app, notify, lock) | `auto` |

**Tiers:**

| Tier | Who approves |
|---|---|
| `auto` | nobody |
| `notify` | nobody; shown afterwards, with undo |
| `chat` | any of your client sessions, via the orchestrator (Telegram buttons, a voice "yes") |
| `device` | a signed approval from an enrolled approver (`axon-desk`, `axon approve`) |
| `local` | a signed approval from **this machine's own** `axon-desk` key. Defined by the key, not by the socket it arrives on: the socket only checks the uid, so the path proves nothing |

**How the tier is set:**
- `policy.toml` maps classes to tiers.
- `grants.toml` (edited locally in V1) can **lower** a tier within policy, for a scope and until an expiry.
- Taint raises a tier.
- A **voice** confirmation counts as `chat` at most, because audio can come from a video or from the assistant itself.

### The approval protocol
1. The daemon builds an `ActionDescriptor`: daemon id, tool, canonical target, exact argv or content hash + size, risk class, program binary hash.
2. It builds the challenge = `hash(descriptor ‖ nonce ‖ daemon_id ‖ account ‖ expiry)`. **There is one nonce per item.**
3. The challenge travels through the orchestrator (event log) to the approvers.
4. The approver **recomputes the hash from the fields it displays** and signs.
   - For a group, possibly spanning several daemons, the approver signs **one list**: `[(daemon, nonce, hash, accept|refuse)…]`, with a single signature.
5. The daemon verifies:
   - the key is in its trusted list
   - the signature
   - the expiry
   - its nonces are unused, and it then marks them used
6. **TOCTOU (time-of-check vs time-of-use) re-check just before execution:** the daemon re-resolves the path and program. If the canonical target or the binary hash changed, it refuses. Files are opened with `O_NOFOLLOW` / `openat2` where possible.
7. Timeout or no answer = deny. A refused item returns `Refused(ask_human)` to the model.

## 7. The event log (orchestrator core)

```rust
enum Event {                                     // durable: stored, numbered by seq
  UserMessage { parts, via: ClientKind },        // text, voice transcript, (V2: files)
  Trigger { source, payload },                   // timer, health alert (proactive)
  ModelMessage { content, tool_calls, usage },
  ToolCallRequested { call_id, machine, tool, args, risk, tier },
  ApprovalRequested { approval_id, items },
  ApprovalResolved { approval_id, decisions, by_device },
  ToolResult { call_id, outcome: Ok|Refused|Failed|TimedOut|Cancelled|Interrupted, body, provenance },
  RolledBack { call_ids, to_seq, by },
  ConversationTainted { source },
  Notice { kind, text },                         // provider down, daemon offline, paused…
  TurnFinished { status: Done|Failed|Cancelled },
}
// ephemeral: broadcast live to subscribed clients, never stored
enum LiveEvent { ModelDelta { text }, ToolProgress { call_id, line } }
```

- **One writer per conversation.** Each conversation is a tokio task that owns an `mpsc::Receiver<Command>`. Clients, daemons, approvers and the scheduler only *send* it commands. It alone appends events with a monotonically increasing `seq`, then broadcasts them. Slow work (model calls, tools) runs in spawned tasks that send their result back as a command, so the writer never waits on anything slow.
- **Streaming:** token deltas are live only. The full `ModelMessage` is stored once the answer is complete. A client that reconnects calls `Subscribe{from_seq}`, gets the stored events, then the live stream.
- **Crash recovery:**
  - **At most once.** The daemon durably records `Started(call_id)` *before* running a call, then its result. Asked about a call again, it returns the stored result if there is one, and `Interrupted` if the call started but never finished (a crash mid-run). It never runs a call twice.
  - At startup, the runtime asks the daemons about any `ToolCallRequested` that has no result. It writes the real result, or `Interrupted` if there is none.
  - Pending approvals expire, which means deny.
- **Versioning:** each stored event carries a schema version, serde variants are tagged by name, and an upcast function per change converts old events.
- **Replay has two modes:**
  - *Deterministic:* recorded model output and tool results, for regression tests.
  - *Live:* a real model against scripted tools, for evals and the injection corpus.
- **Cancel vs rollback:**
  - `Cancel` stops what is running (kills the process, aborts the stream) and records `Cancelled` with partial output.
  - `Rollback` restores checkpoints and records `RolledBack`.
  - Tools declare `reversible()`, which defaults to `false`, and the UI shows "can't be undone" for those.

## 8. The model side (small-model scaffolding)
- `ModelProvider::stream(req) -> Stream<ModelChunk>` + a `Capabilities` profile (tool calling, JSON schema/grammar, vision, context size). One provider per role: `chat`, `summarize`, later `triage`.
- **Constrained output:** tool calls use JSON-schema/grammar decoding when the provider supports it, so a malformed call can't happen.
- **Narrow toolset per turn:** deterministic preselection (keywords/FTS), with a fallback to the full list.
- **Typed results:** `{status, facts, artifacts, raw_excerpt}`.
- **Evidence ledger:** goal, current step, what's confirmed, what failed, what's unverified. Re-injected each round, replacing condensation.
- **Context budget** designed for 8k–32k tokens.
- **Errors to the model:** reason code + friendly message + raw detail.
- **Language:** the system prompt tells the model to answer in the language the user used.
- The offloading features switch on and off through `Capabilities` and per-provider config, never hardcoded. A strong model isn't held back.

## 9. Voice (in `axon-desk`)
- `SttProvider` / `TtsProvider` traits, OpenAI-compatible first (`/v1/audio/transcriptions`, `/v1/audio/speech`), or in-process (whisper.cpp, Piper).
- **Push-to-talk** (V1), then a VAD cut, then STT, then the text goes to the orchestrator.
- `voice.languages = ["fr", "en"]`:
  - one entry = forced;
  - several = detection **restricted to that set**, picking the most likely allowed language.
- **Command words** ("stop", "annule", "cancel", "oui", "non") are matched from a fixed list before the STT guess is used.
- **TTS sentence by sentence** as live deltas arrive. The voice (fr/en) is picked per sentence.
- **Barge-in:** speaking, or saying "stop", interrupts playback. Mic input is ignored while the assistant is speaking (or echo-cancelled).
- A voice "yes" is `chat` tier at most.

## 10. Repo layout
```
axon/
  crates/
    shared/        link + client protocol, Event, ActionDescriptor, RiskClass/Tier, policy schema, crypto helpers
    daemon/        guard, caps/tools, sandbox, checkpoints, grants, approvals verify, link client, local socket
    orchestrator/  core (runtime, event log, conversation tasks, approvals relay, devices, scheduler)
                  providers/ (openai_compat: chat, stt, tts)   adapters/ (client_ws, telegram; V2: openai)
    desk/          axon-desk: tray, chat bar, voice, local approver
    cli/           axon: chat, approve, pair, admin
  docs/          README.md, ARCHITECTURE.md, ROADMAP.md, OPEN_QUESTIONS.md, Vertical_slices/SN.md, decisions/NN-*.md
```
