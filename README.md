# modsh — Modern Shell

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE-APACHE)
[![License: BSL-1.1](https://img.shields.io/badge/License-BSL--1.1-orange.svg)](LICENSE-BSL)
[![Rust](https://img.shields.io/badge/rust-1.87%2B-orange.svg)](https://www.rust-lang.org)
[![Status](https://img.shields.io/badge/status-pre--alpha-red.svg)]()

A POSIX-compatible modern shell with an AI-native context engine. Takes the best from bash, zsh, fish, and nushell — without breaking your scripts.

---

## Why modsh?

| Shell | What modsh inherits |
|---|---|
| bash | POSIX compatibility, scripting reliability |
| zsh | Rich completion, plugin architecture, globbing |
| fish | Autosuggestions, syntax highlighting, human-friendly UX |
| nushell | Structured data pipelines (opt-in) |

Plus: a **local-first AI context engine** that learns your projects, habits, and environment — and compounds over time.

---

## Architecture Overview

```
modsh
├── Core Shell (POSIX-compatible)       Apache-2.0
├── Extended Interactive Layer          Apache-2.0
└── AI Context Engine (HyQAI-ready)     BSL 1.1
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for full design.

---

## AI Context Integration

modsh-ai integrates directly with [`cargo-skill`](https://github.com/SHA888/cargo-skill).
Running `cargo skill` in a modsh project activates the appropriate skill scope for the
current session:

| Command | Effect in modsh-ai |
|---|---|
| `cargo skill lookup <prefix>` | Narrows AI suggestions to that rule domain |
| `cargo skill think` | Activates lookup + reasoning layers |
| `cargo skill write` | Activates all layers — full execution context |
| `cargo skill clear` | Resets to default inference (no skill scope) |

The `.skill/context.md` file written by `cargo-skill` is read by modsh-ai at session
startup. No daemon, no IPC — file-based handoff only.

---

## Licensing

modsh uses a dual-license model:

- **Core shell** (`modsh-core`, `modsh-interactive`): [Apache-2.0](LICENSE-APACHE)
- **AI context engine** (`modsh-ai`): [BSL 1.1](LICENSE-BSL) — free for non-production use; commercial license required for SaaS/enterprise deployment

---

## Status

> Pre-alpha. Not ready for use.

See [TODO.md](TODO.md) for the full roadmap.

---

## Development Setup

```bash
# Prerequisites
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install cargo-audit
cargo install cargo-watch
cargo install cargo-nextest
cargo install cargo-skill

# Clone and build
git clone git@github.com:SHA888/modsh.git
cd modsh
cargo build
```

---

## Contributing

Not open for contributions yet — pre-alpha phase. Watch the repo for updates.

---

## Versioning

Follows [SemVer](https://semver.org). Current: `v0.1.0-alpha`.
