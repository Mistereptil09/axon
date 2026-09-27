# 27/09/2026

## ADR 01: threat model
### 1. **Assets:** what is worth protecting? (Files, keys, the ability to run commands, conversations, your desktop session.)
- Everything user related: files, personal information in the chat, conversations themselves.
- Identification credentials: keys and tokens, and in particular the approver key held by `axon-desk` — losing it doesn't leak data, it lets someone *approve actions*.
- The ability to run commands as the user, which is an asset on its own, separate from whatever files that access happens to touch.
- The desktop session (D-Bus, clipboard, notifications) that `axon-desk` reaches.
- The orchestrator itself, since it is the gateway to the outside world (it communicates with every daemon and every client).

### 2. **Adversaries and how they act:** injected text the model reads; someone on the network; a compromised orchestrator; malicious code inside a repo built; Telegram.
- **a. Injected text the model reads.** The adversary doesn't act directly — it only supplies content (a file, a web page, a tool result) that the model may treat as an instruction instead of as data. The harness must tell the model that external content is untrusted, so it can distinguish a genuine user instruction from text that merely *looks* like one because it sits inside a file the model read (prompt injection).
- **b. Someone on the network.** In V1 the connections between daemons, clients and the orchestrator are not encrypted (no TLS until [S6](../ROADMAP.md#V1)). Someone on the path can read everything (conversations, tool outputs, file contents) and can inject or alter messages after the connection is set up. That makes a network attacker at least as strong as a compromised orchestrator (c), and the same protection applies: `device`/`local` approvals are signed by the approver's key, so they can't be forged or replayed (one nonce per item) even on a plain connection. What V1 doesn't protect against: reading the traffic, and anything that only needs the `chat` tier or `auto`. See the network assumption in §4.
- **c. A compromised orchestrator.** Its effect on accessible daemons must stay limited, which is why it can only *ask* a daemon to act, never act directly. Though if a user is too lax on a daemon's rules (leaves risky classes at `auto`), a compromised orchestrator will still be able to act on the computer through that daemon — a policy misconfiguration risk, not a break of the boundary itself.
- **d. Malicious code inside a built repo.** This isn't only "a script the model decides to run" — code can also run without the model deciding anything, e.g. `.git/hooks`, a `build.rs`, a compiler plugin pulled in by a dependency. That's the exact case Q1 uses as its example (a sandboxed `build.rs` reaching for the daemon socket or session bus). A plain command run also has full user rights and could do harm if unconfined; the safeguard is a sandbox that restricts commands to the accessible files only.
- **e. Telegram.** For now, Telegram would only allow chatting to the AI and acting on low-risk tasks (`chat` tier). More risky operations (write, execute apps) require an approval from a device at the machine.

### 3. **Trust boundaries:** [ARCHITECTURE §4](../ARCHITECTURE.md#4-trust-model) who is trusted for *actions*, who for *data*.
- **a.** You / `axon-desk`: trusted for actions (it signs approvals) and for data.
- **b.** The daemon has full access to the granted paths (create, delete and execute), though only authorized applications can be executed, others are refused.
- **c.** The orchestrator can only ask for the execution of code, never act directly; it isn't trusted for `device`/`local` approvals, only at `chat` tier. It does have access to the files the model has read during a task, because everything passes through its event log — not because it reaches out and reads files itself.
- **d.** The model is never trusted for actions; it sees whatever data it's given.
- **e.** The model provider sees all the data the orchestrator provided, meaning the chat and tool outputs (read files, errors, and application results) — a user choice between a local box and a cloud provider.
- **f.** Telegram is trusted only at `chat` tier; it sees your Telegram conversations, which aren't end-to-end encrypted.
- **g.** Other processes running as you (not sandboxed) are not trusted, but not defended against in V1 — see §4 below.
- For more details see [ARCHITECTURE §4](../ARCHITECTURE.md#4-trust-model).

### 4. **Assumptions and out of scope:** e.g. "malware already running as my user" out of scope in V1, [OPEN QUESTIONS Q4](../OPEN_QUESTIONS.md#q4)).
- As of now the daemon acts and stores secrets (keys, grants, journal) under the user's permissions, which allows any other process already running as that user — a malicious dependency installed by choice, for instance — to read the same files. **Malware already running as your user is out of scope in V1.**
- The fix is splitting the daemon into two processes: a `guardian` that holds the keys, grants and journal and verifies approvals, and an `executor` running as you with nothing secret that touches files and runs sandboxed commands. That's two processes, two sockets and a system-plus-user service pair — too much for V1, so it's recorded here as a known limitation. Tracked as [Q4](../OPEN_QUESTIONS.md#q4).
- **V1 assumes a trusted network.** Connections between daemons, clients and the orchestrator have no TLS, so they must only run over localhost, a LAN you control, or a VPN such as Tailscale (which then provides the encryption). Adding TLS, with the orchestrator's key pinned as described in [ARCHITECTURE §3](../ARCHITECTURE.md#3-connections), is what would take this assumption away.

### 5. **Open questions:** [OPENS QUESTIONS](../OPEN_QUESTIONS.md) Q1–Q3.
- **Q1.** Sandboxed children may be able to reach the daemon socket and session bus via `$XDG_RUNTIME_DIR`, since both only check uid. Whether Landlock on this kernel blocks that is unverified; deferred to S3.
- **Q2.** Btrfs snapshots are put aside, since most distros default to ext4 rather than btrfs, so a filesystem-specific layer isn't a good bet to rely on. A shadow git repository is used instead for **checkpoints** — but that isn't a like-for-like replacement: checkpoints only cover writes the daemon's own tools make inside granted roots, not side effects of `run` outside those roots (a script that writes elsewhere, installs a package, or edits a dotfile). That gap is accepted for V1 rather than assumed away; revisit if a portable equivalent turns up.
- **Q3.** Files the model can access could potentially contain prompt injections, as such we consider that such files "taint" the conversation and raise the security requirements when the AI asks for permissions. Still need to define what sources "taint" a chat.
