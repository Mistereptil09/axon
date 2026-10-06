# Research: existing harnesses (2026-09)

> Docs: [README](../README.md) · [OPEN QUESTIONS](../OPEN_QUESTIONS.md)

**Takeaway:** the popular assistants (OpenClaw, Hermes) chase capability and trust the model; their failures are prompt injection and malicious skills. Axon's bet, the harness owns every guarantee, is the gap.

| Harness | Worth taking | Lesson |
|---|---|---|
| **OpenClaw** | Approvals only tighten config; allowlist by binary path + argv regex; "once" vs "always"; no approver = deny; inline-eval detection; script hash binding | ~1 in 8 ClawHub skills were malicious (ClawHavoc): a skill is code, keep policy human-edited |
| **Codex CLI** | Separate *sandbox* (what the OS allows) from *approval* (when to ask); Landlock + seccomp blocking network; `debug landlock` command | — |
| **Claude Code** | Sandbox network only through an outside proxy with a domain allowlist | Files and network must both be isolated |
| **Hermes Agent** | Memory tiers: small `USER.md`/`MEMORY.md` always loaded + SQLite FTS5 search; prompt ordered stable → volatile for caching | Fits small models, no vector DB |
| **Goose** | Show loaded instructions to the user | Red team won with zero-width Unicode in a calendar invite |
| **CaMeL** (DeepMind) | Planner model never sees untrusted text, only references to it | Long-term form of taint |

- **"The Attacker Moves Second"** (OpenAI, Anthropic, DeepMind): 12 injection filters all fell to adaptive attacks. Don't rely on classifiers.
- **Meta's Rule of Two:** a session may have at most two of untrusted input, private data, outside effects.
- **[Source study of 11 harnesses](https://arxiv.org/pdf/2609.00006):** none use vector retrieval, and behaviour rules are moving from prompts to config.

Resulting questions: [Q5–Q9](../OPEN_QUESTIONS.md#q5-what-an-approved-command-really-runs).
