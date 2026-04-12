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

- [ ] **Dev tooling**
  - [ ] `cargo install cargo-audit`
  - [ ] `cargo install cargo-watch`
  - [ ] `cargo install cargo-nextest`
  - [ ] `cargo install cargo-skill`
  - [ ] Add `pre-commit` config (fmt, clippy, audit)

---

## Phase 1 — Core Shell (`v0.1.0`)

### 1.1 Lexer (`modsh-core`)
- [ ] Define token types (Word, Operator, Redirect, etc.)
- [ ] Implement tokenizer for POSIX sh syntax
- [ ] Handle quoting (`'`, `"`, `\`)
- [ ] Handle heredoc tokens
- [ ] Handle comment stripping
- [ ] Unit tests — token stream correctness
- [ ] Fuzz testing — malformed input safety

### 1.2 Parser
- [ ] Define AST node types
  - [ ] SimpleCommand
  - [ ] Pipeline
  - [ ] List (AND/OR)
  - [ ] CompoundCommand (if/for/while/case/subshell)
  - [ ] FunctionDefinition
- [ ] Implement recursive descent parser
- [ ] Error recovery — parse partial input gracefully
- [ ] Unit tests — AST correctness per POSIX grammar

### 1.3 Expander
- [ ] Parameter expansion (`$VAR`, `${VAR:-default}`, etc.)
- [ ] Command substitution (`$(cmd)`, backticks)
- [ ] Arithmetic expansion (`$((expr))`)
- [ ] Word splitting
- [ ] Glob/pathname expansion
- [ ] Tilde expansion
- [ ] Unit tests — expansion edge cases

### 1.4 Executor
- [ ] Fork/exec pipeline
- [ ] Pipe setup (`|`)
- [ ] Redirect handling (`>`, `>>`, `<`, `2>`, `&>`)
- [ ] Builtin dispatch
- [ ] Exit status propagation
- [ ] Subshell execution
- [ ] Unit tests — execution correctness

### 1.5 Builtins
- [ ] `cd`, `pwd`
- [ ] `export`, `unset`, `env`
- [ ] `alias`, `unalias`
- [ ] `source` / `.`
- [ ] `echo`, `printf`
- [ ] `exit`, `return`
- [ ] `set`, `shift`
- [ ] `test` / `[`
- [ ] `read`
- [ ] `trap`

### 1.6 Job Control
- [ ] Foreground/background execution (`&`)
- [ ] `jobs`, `fg`, `bg`
- [ ] SIGINT, SIGTERM, SIGCHLD, SIGHUP handling
- [ ] Process group management

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
