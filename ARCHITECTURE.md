# modsh — Architecture

**Version:** 0.1.0-alpha  
**Last updated:** 2026-04-12

---

## Design Principles

1. **POSIX-first** — existing scripts run unmodified
2. **Local-first** — AI context lives on the machine, never required to phone home
3. **Layered** — each layer is independently useful; AI layer is opt-in
4. **Rust-native** — memory safe, fast startup, single binary distribution

---

## Workspace Structure

```
modsh/                          # Cargo workspace root
├── modsh-core/                 # POSIX-compatible shell core       [Apache-2.0]
├── modsh-interactive/          # Extended interactive layer        [Apache-2.0]
├── modsh-ai/                   # AI context engine                 [BSL 1.1]
└── modsh-cli/                  # Binary entrypoint                 [Apache-2.0]
```

---

## Layer 1 — Core Shell (`modsh-core`)

**License:** Apache-2.0  
**Responsibility:** POSIX sh compliance

### Components

| Component | Description |
|---|---|
| Lexer | Tokenizes POSIX shell syntax |
| Parser | Builds AST from token stream |
| Expander | Variable, glob, and command expansion |
| Executor | Forks/execs commands, manages pipes |
| Builtins | `cd`, `export`, `source`, `alias`, etc. |
| Job control | Foreground/background, signals |

### Key constraints
- Must pass the POSIX sh test suite
- No allocation in hot paths where avoidable
- Zero unsafe code in core parsing paths

---

## Layer 2 — Extended Interactive Layer (`modsh-interactive`)

**License:** Apache-2.0  
**Responsibility:** Modern interactive experience on top of the POSIX core

### Components

| Component | Description |
|---|---|
| Line editor | Readline-compatible, cursor movement, history |
| Syntax highlighter | Real-time token coloring |
| Autosuggester | Fish-style ghost text from history + context |
| Completer | zsh-style tab completion with descriptions |
| Prompt engine | Configurable, async-safe prompt |
| History engine | Structured history with metadata (directory, exit code, duration) |
| Plugin system | WASM-based plugin sandbox |

### Structured output (opt-in)
- Nushell-style typed pipeline values
- Falls back to raw text for POSIX compatibility
- Enabled per-command via `--structured` flag or config

---

## Layer 3 — AI Context Engine (`modsh-ai`)

**License:** BSL 1.1  
**Responsibility:** Local-first intelligence that compounds over time

### Context graph

```
ContextGraph
├── ProjectNode       (detected project type, stack, git remote)
├── CommandNode       (command, args, directory, exit code, duration)
├── PatternNode       (recurring command sequences)
├── ServerNode        (SSH hosts, their typical commands)
└── ErrorNode         (failed commands + what fixed them)
```

### Inference pipeline

```
User input
    ↓
ContextRetriever    →  pulls relevant nodes from graph
    ↓
LocalLLM            →  llama.cpp / ollama sidecar (offline-first)
    ↓
Suggester           →  ranked completions with explanations
    ↓
User accepts/rejects →  feedback updates graph weights
```

### Key properties
- Fully offline by default — no API calls required
- Optional cloud sync via encrypted context export
- Model-agnostic — any GGUF-compatible model works
- Context stored in SQLite (`~/.local/share/modsh/context.db`)

---

## Binary entrypoint (`modsh-cli`)

**License:** Apache-2.0

Wires all three layers together. Handles:
- CLI argument parsing
- Config loading (`~/.config/modsh/config.toml`)
- Layer initialization order
- Signal handling (SIGINT, SIGTERM, SIGHUP)

---

## Data Flow

```
keystroke
    → modsh-interactive (line editor)
    → modsh-ai (context-aware suggestion, async)
    → user confirms input
    → modsh-core (parse + execute)
    → output
    → modsh-ai (observe result, update context graph)
```

---

## Config File

Location: `~/.config/modsh/config.toml`

```toml
[shell]
posix_strict = false         # strict POSIX mode for scripting
history_size = 50000

[interactive]
syntax_highlight = true
autosuggestions = true
structured_output = false    # opt-in nushell-style pipelines

[ai]
enabled = true
offline = true               # never phone home
model = "auto"               # auto-detect from ollama/llama.cpp
context_db = "~/.local/share/modsh/context.db"

[theme]
prompt = "default"           # or path to custom prompt script
```

---

## Dependency Strategy

| Crate | Purpose |
|---|---|
| `rustyline` | Line editing foundation |
| `pest` or `winnow` | Parsing |
| `tokio` | Async runtime (AI inference, suggestions) |
| `rusqlite` | Context graph storage |
| `serde` + `toml` | Config serialization |
| `clap` | CLI argument parsing |
| `crossterm` | Terminal control, syntax highlighting |

All dependencies pinned to latest stable at time of release.

---

## Versioning

| Version | Milestone |
|---|---|
| v0.1.0 | Core shell — POSIX-compatible execution |
| v0.2.0 | Interactive layer — highlighting, completion, history |
| v0.3.0 | Plugin system |
| v0.4.0 | Structured output (opt-in) |
| v0.5.0 | AI context engine — local inference |
| v0.6.0 | Context sync across machines |
| v1.0.0 | Stable, documented, tested |
