# Learning note: validating data

> Docs: [README](../README.md) · [ROADMAP](../ROADMAP.md) · [S0](../Vertical_slices/S0.md)

## The short version

**Check data once, at the door, then trust it everywhere inside.**

"The door" is where data enters the program from outside: a config file, the model's output, a network message. You check it there, turn it into a type that can only hold good values, and the rest of the code never checks again.

You already did this: `RawPaths` (unchecked, from the file) becomes `Paths` (checked) in `resolve()`. Everything below is the same idea, applied to other kinds of rules.

There are three kinds of rules, and each is checked at a different moment:

| The rule needs… | Example | Checked | Tool |
|---|---|---|---|
| only the value itself | "between 1 and 5" | while parsing the file | a type: std, hand-written, or `nutype` |
| the value + outside information | "inside one of the workspaces" | once, in `resolve` | a constructor that takes the information as a parameter |
| the value + something that can change later | "this file is still inside the workspace" | when the action happens | the operation itself (S1/S2 capability handles) |

**For S0 you need none of the new parts.** This note is for when they come up (S1, S5).

---

## Part 1: rules that only need the value

### Step 0: pick the right standard type

Before writing anything, check whether a standard type already says it. The type *is* the check, and serde enforces it while parsing:

```rust
use std::num::NonZeroU8;

#[derive(serde::Deserialize)]
struct RawConfig {
    port: u16,                  // refuses -1, 70000, "abc"
    verbose: bool,              // refuses "yes", 1
    tool_attempts: NonZeroU8,   // refuses 0 (and anything above 255)
}
```

This covers most cases. Only go further when no standard type fits.

### Step 1: a hand-written "newtype"

A **newtype** is a struct with one field that wraps an existing type to add a rule. In Python it would look like this:

```python
class ToolAttempts:
    def __init__(self, value: int):
        if not 1 <= value <= 5:
            raise ValueError(f"tool_attempts must be between 1 and 5, got {value}")
        self._value = value   # "_" is only a convention: anyone can still change it
```

The Rust version:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "u8")]     // (3) serde must go through try_from below: no way around the check
pub struct ToolAttempts(u8);  // (1) the field has no `pub`: code outside this file can't write ToolAttempts(99)

// (2) The only way to build a ToolAttempts: this is where the rule lives.
impl TryFrom<u8> for ToolAttempts {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (1..=5).contains(&value) {
            Ok(ToolAttempts(value))
        } else {
            Err(format!("tool_attempts must be between 1 and 5, got {value}"))
        }
    }
}

impl ToolAttempts {
    /// Read the number back.
    pub fn get(self) -> u8 {
        self.0   // `.0` is the first (and only) field
    }
}
```

What each piece does:
1. **Private field.** In Python, `_value` is a polite request. In Rust, a field without `pub` really can't be touched outside its module (here, outside this file). So nobody can skip the check by writing `ToolAttempts(99)`.
2. **`TryFrom`** is the standard name for "a conversion that can fail". Building one from code: `ToolAttempts::try_from(3)`.
3. **`#[serde(try_from = "u8")]`** tells serde: "read a `u8` from the file, then pass it through `try_from`". So `tool_attempts = 0` in `config.toml` is refused by `load`, with the line number.

Once a function receives a `ToolAttempts`, it knows the value is between 1 and 5. It never checks again.

### Step 2: the shortcut, `nutype`

Writing Step 1 for every type gets repetitive. The `nutype` crate writes it for you:

```rust
use nutype::nutype;

#[nutype(
    validate(greater_or_equal = 1, less_or_equal = 5),   // the rule
    derive(Debug, Clone, Copy, Deserialize),             // what you'd normally #[derive]
)]
pub struct ToolAttempts(u8);

// Using it:
//   ToolAttempts::try_new(3)   → Ok(...)
//   ToolAttempts::try_new(0)   → Err(...)
//   attempts.into_inner()      → the u8 back
```

It generates the same three pieces as Step 1: private field, a checking constructor, serde support. It needs `nutype = { version = "0.8.0", features = ["serde"] }`. It also handles strings (`not_empty`, `len_char_max = 64`), trimming (`sanitize(trim)`) and custom rules (`predicate = |v| ...`).

---

## Part 2: rules that need outside information

"Is this path inside a workspace?" can't be answered by looking at the path alone: you also need **the list of workspaces**, which comes from `policy.toml`. `try_from` only receives the value, so it can't do this.

The fix is simple: a constructor that takes the extra information as a parameter.

```rust
use std::path::{Path, PathBuf};

pub struct Workspaces {
    roots: Vec<PathBuf>,   // e.g. ["/home/me/proj", "/home/me/notes"], from policy.toml
}

pub enum Denial {
    OutsideWorkspaces,
}

pub struct SafePath(PathBuf);   // private field again: `SafePath::new` is the only way in

impl SafePath {
    /// The outside information (`workspaces`) is just a parameter.
    pub fn new(workspaces: &Workspaces, raw: PathBuf) -> Result<SafePath, Denial> {
        // Python: any(raw.is_relative_to(root) for root in workspaces.roots)
        let inside = workspaces.roots.iter().any(|root| raw.starts_with(root));

        if inside {
            Ok(SafePath(raw))
        } else {
            Err(Denial::OutsideWorkspaces)
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
```

(Simplified on purpose: the real check in S1 also resolves `..` and symlinks first.)

**Where to call it: in `resolve`**, the same place you already turn `RawPaths` into `Paths`. Parsing handles Part 1 rules; `resolve` handles Part 2 rules, because that's where the outside information is available:

```rust
impl RawConfig {
    pub fn resolve(self, workspaces: &Workspaces) -> Result<Config, ResolveError> {
        Ok(Config {
            paths: self.paths.resolve(App::Daemon)?,              // needs the environment
            log_file: SafePath::new(workspaces, self.log_file)?,  // needs the policy
            tool_attempts: self.tool_attempts,                    // already checked while parsing: just copied
        })
    }
}
```

The whole journey:

```
config.toml ──load──▶ RawConfig ──resolve(context)──▶ Config ──▶ the rest of the program
            Part 1 rules          Part 2 rules                     never checks again
            (types, nutype)       (constructors with context)
```

### Why not check when the value is read?

You could write a getter that checks every time (`fn log_file(&self) -> Result<&Path, Denial>`). Avoid it:
- A typo in `config.toml` would only show up later, in the middle of some unrelated task, instead of at startup.
- Every caller has to handle an error at every use.

---

## Part 3: rules about things that can change

`SafePath` proves the path was fine *when it was checked*. A millisecond later, a folder in that path could be replaced by a symlink to `/etc`. No check done in advance can prevent that, whether in `resolve` or in a getter. This is the TOCTOU problem (Time Of Check To Time Of Use).

The only fix is to make the check part of the action: open files relative to an already-open workspace folder, so the kernel itself refuses to leave it. That's the "capability handles" of S1/S2. You don't need it before then.

---

## Is all this worth it?

**Not everywhere.** Wrap values only where data crosses a door *and* a wrong value would cause real harm:

| Worth a checked type | Leave as a plain type |
|---|---|
| paths the daemon acts on (security) | a display name, a free-text note |
| anything the model produces (untrusted) | internal values your own code computes |
| values with a real rule (`tool_attempts ≥ 1`) | fields with no rule at all (`verbose: bool`) |

Most of Axon's data never needs more than Step 0. And no custom library is needed on top of this: `nutype` already *is* that library, and `resolve` is a pattern, not something to build.
