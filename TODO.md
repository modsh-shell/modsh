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
- [ ] **BLOCKING:** Wire expander into executor — expand command arguments, for-loop words, case patterns (see §1.8)

### 1.4 Executor
- [x] Fork/exec pipeline
- [x] Pipe setup (`|`)
- [x] Redirect handling — < > >> 2> 2>> &> &>> heredoc herestring all done
- [x] Builtin dispatch
- [x] Exit status propagation
- [x] Background execution (&) — true fork with process groups, job tracking
- [x] Subshell execution — true fork with waitpid, proper exit status propagation
- [x] Unit tests — 40+ tests covering commands, pipelines, operators, subshells, background, builtins
- [ ] **BLOCKING:** Implement for-loop variable binding — currently drops loop variable (see §1.8)
- [ ] **BLOCKING:** Implement case-statement pattern matching — currently runs all clauses (see §1.8)

### 1.5 Builtins
- [x] `cd`, `pwd`
- [x] `export`, `unset`, `env`
- [x] `alias`, `unalias`
- [x] `source` / `.` — executes scripts in current shell context
- [x] `echo`
- [x] `printf`
- [x] `exit`
- [x] `return`
- [x] `set`, `shift`
- [x] `test` / `[`
- [x] `read`
- [x] `trap`
- [ ] **BLOCKING:** `break`, `continue` — required for loops with early exit (gate for Phase 2)
- [ ] **BLOCKING:** `exec` — required for shebang script delegation (gate for Phase 2)
- [ ] `eval` — evaluate string as commands
- [ ] `wait` — wait for background jobs to complete

### 1.6 Job Control
- [x] Foreground/background execution — tcsetpgrp, waitpid, killpg(SIGCONT) implemented
- [x] `jobs`, `fg`, `bg` — builtins implemented with job spec parsing
- [x] Signal handling — SIGCHLD (reap), SIGINT/SIGQUIT (ignore in interactive) installed
- [x] Process group management — setpgid, tcsetpgrp wired into foreground/background

### 1.7 POSIX Compliance
- [x] Run against POSIX sh test suite — 29 integration tests in `modsh-cli/tests/posix.rs`, 19 passing
- [x] Document known deviations — see `POSIX.md` for 13 documented deviations

### 1.8 Correctness Fixes — Phase 1 Completion

**BLOCKING — must fix before v0.1.0 final (affects all script execution modes)**

- [ ] **Wire expander into executor** — `modsh-core/src/executor.rs:execute_simple()`
  - Root cause: `execute_simple` passes raw token strings to external commands and builtins without calling `Expander::expand()`
  - Impact: Variables like `$HOME` do not expand in command arguments; for-loop words; case patterns; while conditions
  - Concrete failure: `echo $HOME` prints empty line instead of home directory path
  - Code path: `execute_simple()` lines 496-635 must call expander before dispatch
  - Affects: 6 of 9 ignored POSIX tests

- [ ] **Implement for-loop variable binding** — `modsh-core/src/executor.rs:execute_for()` lines 213-229
  - Root cause: Loop variable is explicitly discarded (`let _ = word;` line 226)
  - Impact: Every iteration of a for loop runs with the same (pre-loop) environment
  - Concrete failure: `for x in a b c; do echo $x; done` prints three empty lines instead of `a`, `b`, `c`
  - Fix: Store `word` value into `state.env` before executing loop body

- [ ] **Implement case-statement pattern matching** — `modsh-core/src/executor.rs:execute_case()` lines 247-259
  - Root cause: No pattern matching logic; all clause bodies execute unconditionally
  - Impact: All branches of a case statement run; only first matching pattern should run
  - Concrete failure: `case $x in a) echo A;; b) echo B;; esac` prints both `A` and `B`
  - Fix: Implement POSIX pattern matching (glob-style) and break after first match

- [ ] **Fix `--file` mode script parsing** — `modsh-cli/src/main.rs:run_script()` lines 86-99
  - Root cause: Iterates `content.lines()` and executes each non-blank line individually
  - Impact: Any multiline construct (if/for/while/case/function/heredoc) is broken
  - Concrete failure: Script with `if true; then\n  echo yes\nfi` fails to parse
  - Fix: Use streaming parser approach (e.g., accumulate lines until parser.is_complete())
  - Blocks: `.modshrc` loading, script files with control flow

- [ ] **Implement `--stdin` script execution** — `modsh-cli/src/main.rs:run_stdin()` lines 184-197
  - Root cause: Reads stdin but execution loop body is empty (TODO stub)
  - Impact: Piping scripts to modsh produces no output
  - Concrete failure: `echo 'echo hello' | modsh` produces no output
  - Fix: Implement accumulator pattern (similar to `--file` fix) to handle streaming input

**RECOMMENDING — should fix before v0.1.0 is recommended for interactive use**

- [ ] Fix `builtin_trap` custom command handler — `modsh-core/src/builtins.rs` lines 993-995
  - Root cause: `trap CMD SIGNAL` form registers handler string but never executes it
  - Impact: Error-handling traps (`trap cleanup EXIT`) silently no-op
  - Concrete failure: Scripts relying on trap cleanup do not clean up on exit

- [ ] Fix `fg` spin-loop race condition — `modsh-core/src/jobcontrol.rs` lines 173-226
  - Root cause: Uses `WNOHANG` in spin loop instead of blocking `waitpid`
  - Impact: `fg` can return before foreground job actually exits
  - Fix: Use blocking `waitpid(WUNTRACED)` on first call, then `WNOHANG` for subsequent checks

- [ ] Fix `builtin_read` IFS handling — `modsh-core/src/builtins.rs` line 871
  - Root cause: Uses `split_whitespace()` instead of consulting `state.env["IFS"]`
  - Impact: Custom IFS does not apply to `read` builtin
  - Concrete failure: `IFS=: read a b <<< "x:y"` does not split on `:`

- [ ] Update POSIX.md stale documentation — `POSIX.md` line 207
  - Current: "19 tests passing, 10 known failures"
  - Actual: 20 tests passing, 9 known failures (case_stmt was recently fixed)

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
