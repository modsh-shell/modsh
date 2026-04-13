# modsh — TODO

**Format:** tasks → subtasks  
**Versioning:** SemVer  
**Status tags:** `[ ]` todo · `[x]` done · `[~]` in progress · `[!]` blocked

---

## Phase 0 — Project Bootstrap (`v0.0.1`)

- [x] **Repository setup**
  - [x] Initialize Cargo workspace with 4 crates
  - [x] Add `LICENSE-APACHE` (Apache-2.0)
  - [x] Add `LICENSE-BSL` (BSL 1.1)
  - [x] Add `.gitignore`
  - [x] Add `.cargo/config.toml` with workspace settings
  - [x] Configure `rust-toolchain.toml` (stable channel)

- [x] **CI/CD bootstrap**
  - [x] Add GitHub Actions workflow — `ci.yml`
    - [x] `cargo check`
    - [x] `cargo test`
    - [x] `cargo clippy -- -D warnings`
    - [x] `cargo fmt --check`
    - [x] `cargo audit`
  - [x] Add `dependabot.yml` for automated dependency updates
  - [x] Add PR template (`.github/pull_request_template.md`)
  - [x] Add issue templates (bug, feature)

- [x] **Dev tooling**
  - [x] `cargo install cargo-audit`
  - [x] `cargo install cargo-watch`
  - [x] `cargo install cargo-nextest`
  - [x] `cargo install cargo-skill`
  - [x] Add `pre-commit` config (fmt, clippy, audit)
  - [ ] Add `AGENTS.md` at workspace root
    - Cross-agent contract: which crate owns what, where AI suggestions write output,
      verification requirement before a suggestion surfaces to the user
    - Specifies: output paths (`~/.local/share/modsh/sessions/`), skill loader
      discovery order, session file naming convention (`<remote>-<branch>`)

---

## Phase 1 — Core Shell (`v0.1.0`)

### 1.1 Lexer (`modsh-core`)
- [x] Define token types (Word, Operator, Redirect, etc.)
- [x] Implement tokenizer for POSIX sh syntax
- [x] Handle quoting (`'`, `"`, `\`)
- [x] Handle heredoc tokens — delimiter and body reading done
- [x] Handle comment stripping
- [x] Unit tests — comprehensive edge cases covered (24 tests)
- [x] Fuzz testing — cargo-fuzz setup with lexer target (ran 37k+ iterations without crash)

### 1.2 Parser
- [x] Define AST node types
  - [x] SimpleCommand
  - [x] Pipeline
  - [x] List (AND/OR)
  - [x] CompoundCommand — if/for/while/case/subshell/group all done
  - [x] FunctionDefinition — POSIX (name() { }) and bash (function name { }) forms
- [x] Implement recursive descent parser
- [x] Error recovery — parse_partial() with is_incomplete detection, 14 tests
- [x] Unit tests — 53 parser tests covering POSIX grammar comprehensively

### 1.3 Expander
- [x] Parameter expansion — all POSIX operators: $VAR, ${VAR}, ${VAR:-default}, ${VAR:=assign}, ${VAR:?err}, ${VAR:+alt}, plus colon variants (treats empty as unset)
- [x] Command substitution (`$(cmd)`, `` `cmd` ``)
- [x] Arithmetic expansion (`$((expr))`)
- [x] Word splitting — IFS-based, handles spaces/tabs/newlines, custom IFS, empty IFS (no split)
- [x] Glob/pathname expansion — *, ?, [abc] patterns, no-match-returns-pattern behavior
- [x] Tilde expansion — ~/ (current user), ~user (other users on Unix via libc getpwnam)
- [x] Unit tests — basic tests + 80+ edge case tests for lexer, parser, expander

### 1.4 Executor
- [x] Fork/exec pipeline
- [x] Pipe setup (`|`)
- [~] Redirect handling — basic < > >> done, need 2>>, &>, heredocs
- [x] Builtin dispatch
- [x] Exit status propagation
- [ ] Background execution (&) — runs sync, need true fork
- [ ] Subshell execution — runs sync, need fork
- [ ] Unit tests — execution correctness

### 1.5 Builtins
- [x] `cd`, `pwd`
- [x] `export`, `unset`, `env`
- [ ] `alias`, `unalias`
- [~] `source` / `.` — reads file, TODO: actually execute
- [x] `echo`
- [ ] `printf`
- [x] `exit`
- [ ] `return`
- [ ] `set`, `shift`
- [ ] `test` / `[`
- [ ] `read`
- [ ] `trap`

### 1.6 Job Control
- [~] Foreground/background execution — data structures done, need fork/signals
- [~] `jobs`, `fg`, `bg` — stubs exist, need terminal control
- [ ] Signal handling — SIGINT, SIGTSTP, SIGCHLD, SIGHUP
- [ ] Process group management — need setpgid, tcsetpgrp

### 1.7 POSIX Compliance
- [ ] Run against POSIX sh test suite
- [ ] Document known deviations (if any)

---

## Phase 2 — Interactive Layer (`v0.2.0`)

### 2.1 Line Editor (`modsh-interactive`)
- [ ] Integrate `rustyline` or custom line editor
- [ ] Cursor movement (word-level, line-level)
- [ ] Kill/yank (Ctrl+K, Ctrl+Y)
- [ ] History search (Ctrl+R)
- [ ] Multi-line editing

### 2.2 Syntax Highlighter
- [ ] Real-time token coloring
  - [ ] Commands (green if found in PATH, red if not)
  - [ ] Arguments
  - [ ] Strings
  - [ ] Operators
  - [ ] Errors
- [ ] Configurable color scheme

### 2.3 Autosuggestions
- [ ] Ghost text from history (fish-style)
- [ ] Accept with right arrow / End key
- [ ] Partial accept with Ctrl+Right (word)
- [ ] Suppress when suggestion is irrelevant

### 2.4 Completion Engine
- [ ] Command name completion
- [ ] Path completion
- [ ] Flag completion (from `--help` parsing)
- [ ] Git-aware completion
- [ ] Completion descriptions (zsh-style)
- [ ] Async completion (non-blocking)

### 2.5 Prompt Engine
- [ ] Async prompt rendering (no blocking on git status)
- [ ] Default prompt (user, host, path, git branch, exit code)
- [ ] Configurable prompt via config or script
- [ ] Right-prompt support

### 2.6 History Engine
- [ ] Structured history entries (command, directory, exit code, duration, timestamp)
- [ ] History deduplication
- [ ] History search with fuzzy matching
- [ ] Per-project history filtering
- [ ] History export/import

---

## Phase 3 — Plugin System (`v0.3.0`)

- [ ] Define plugin API
- [ ] WASM-based plugin sandbox
- [ ] Plugin manifest format (`modsh-plugin.toml`)
- [ ] Plugin loader
- [ ] Built-in plugin manager (`modsh plugin install/remove/list`)
- [ ] Example plugin: `git-modsh`
- [ ] Plugin documentation

---

## Phase 4 — Structured Output (`v0.4.0`)

- [ ] Define typed value system (string, int, float, bool, list, table, null)
- [ ] Opt-in structured pipeline (`cmd --structured | filter col`)
- [ ] Table renderer for terminal output
- [ ] JSON/CSV export from structured pipelines
- [ ] Fallback to raw text for POSIX compatibility
- [ ] Integration tests — structured + POSIX pipelines coexist

---

## Phase 5 — AI Context Engine (`v0.5.0`) [BSL 1.1]

### 5.1 Context Graph (`modsh-ai`)
- [ ] Define graph schema (SQLite via `rusqlite`)
- [ ] ProjectNode — detect project type from filesystem
- [ ] CommandNode — observe and store command executions
- [ ] PatternNode — detect recurring sequences
- [ ] ServerNode — SSH host awareness
- [ ] ErrorNode — failed command + recovery tracking
- [ ] Graph query API

### 5.2 Local Inference
- [ ] Ollama integration (primary)
- [ ] llama.cpp sidecar fallback
- [ ] Model auto-detection
- [ ] Async suggestion pipeline (non-blocking)
- [ ] Suggestion ranking

### 5.3 Context Retriever
- [ ] Retrieve relevant context for current input
- [ ] Weight by recency, frequency, project
- [ ] Cap context window for LLM efficiency

### 5.4 Feedback Loop
- [ ] Accept/reject tracking
- [ ] Weight adjustment on feedback
- [ ] Periodic graph pruning

### 5.5 Privacy
- [ ] All inference local by default
- [ ] Explicit opt-in for any network activity
- [ ] Context db encryption at rest (optional)
- [ ] `modsh context purge` command

### 5.6 Skill Loader
- [ ] Implement skill file discovery in priority order:
  - [ ] `.skill/context.md` (cargo-skill active session)
  - [ ] `.modsh/skills/*.md` (project-local)
  - [ ] `~/.local/share/modsh/skills/*.md` (user-level)
- [ ] Load only active scope — not all skill files per invocation
- [ ] Skip load (no-op) if no skill files found — no error
- [ ] Unit tests — discovery order, missing files, empty context

### 5.7 Session Memory
- [ ] Define session file path: `~/.local/share/modsh/sessions/<slug>.md`
  - Slug derived from git remote + branch; fallback to hostname + cwd hash
- [ ] Write session file: last N commands, inferred project context, user corrections
- [ ] Read session file on startup before first inference call
- [ ] Append-only writes during session; no full rewrites
- [ ] `modsh context session clear` — delete current session file
- [ ] `modsh context session show` — print active session file path + line count
- [ ] Unit tests — slug derivation, write/read round-trip, clear

---

## Phase 6 — Cross-machine Sync (`v0.6.0`) [BSL 1.1]

- [ ] Context export format (encrypted JSON)
- [ ] chezmoi-compatible sync strategy
- [ ] SSH-based sync (no cloud dependency)
- [ ] Optional cloud sync (E2E encrypted)
- [ ] Conflict resolution strategy
- [ ] `modsh sync push/pull/status` commands

---

## Phase 7 — Stable Release (`v1.0.0`)

- [ ] **Documentation**
  - [ ] User guide (mdBook)
  - [ ] Plugin authoring guide
  - [ ] Config reference
  - [ ] Migration guide from bash/zsh/fish

- [ ] **Distribution**
  - [ ] Publish `modsh-core` to crates.io
  - [ ] Publish `modsh-interactive` to crates.io
  - [ ] AUR package (`modsh`)
  - [ ] Homebrew formula
  - [ ] Debian/Ubuntu `.deb` package
  - [ ] Binary releases via GitHub Actions

- [ ] **Quality**
  - [ ] 80%+ test coverage on core + interactive
  - [ ] Fuzzing CI integration
  - [ ] Performance benchmarks (startup time < 50ms)
  - [ ] Security audit (`cargo audit` clean)

- [ ] **Community**
  - [ ] CONTRIBUTING.md
  - [ ] Code of conduct
  - [ ] GitHub Discussions enabled
  - [ ] Changelog (Keep a Changelog format)

---

## Ongoing — DevSecOps

- [ ] `cargo audit` runs on every PR
- [ ] Dependency review on every `dependabot` PR
- [ ] SBOM generation on releases
- [ ] Signed releases (GPG or Sigstore)
- [ ] Security policy (`SECURITY.md`)
