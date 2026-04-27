# POSIX Compliance — modsh

**Version:** 0.1.0-alpha  
**Last updated:** 2026-04-27

---

## Overview

modsh targets POSIX.1-2024 (IEEE Std 1003.1-2024) shell compliance. This document tracks implemented features, known deviations, and planned work.

---

## Implemented POSIX Features

### Lexer / Parser

| Feature | Status | Notes |
|---|---|---|
| Simple commands | ✅ | Words, assignments, redirects |
| Pipelines | ✅ | `\|` with proper pipe setup |
| Lists (AND/OR) | ✅ | `&&`, `\|\|` |
| Sequential lists | ✅ | `;` and newlines |
| Grouping commands | ✅ | `( ... )` subshells, `{ ... }` groups |
| If statements | ✅ | `if ... then ... elif ... else ... fi` |
| For loops | ✅ | `for name [in word ...] do ... done` |
| While loops | ✅ | `while ... do ... done` |
| Case statements | ✅ | `case word in [(]pattern) ... esac` |
| Function definitions | ✅ | POSIX `name() compound-command` and bash `function name` |
| Background commands | ✅ | `command &` with fork and process groups |
| Heredocs | ✅ | `<<` and `<<-` delimiter and body reading |
| Herestrings | ✅ | `<<< word` |
| Comments | ✅ | `#` to end of line |

### Expander

| Feature | Status | Notes |
|---|---|---|
| Parameter expansion | ✅ | `$var`, `${var}`, `${var:-default}`, `${var:=assign}`, `${var:?err}`, `${var:+alt}`, colon variants |
| Command substitution | ✅ | `$(cmd)` and `` `cmd` `` |
| Arithmetic expansion | ✅ | `$((expr))` |
| Tilde expansion | ✅ | `~/` current user, `~user` via `getpwnam` |
| Word splitting | ✅ | IFS-based, custom IFS, empty IFS |
| Glob expansion | ✅ | `*`, `?`, `[abc]`, no-match-returns-pattern |

### Redirections

| Feature | Status | Notes |
|---|---|---|
| Input `<` | ✅ | |
| Output `>` | ✅ | |
| Append `>>` | ✅ | |
| stderr redirect `2>`, `2>>` | ✅ | |
| Combined `&>`, `&>>` | ✅ | bash extension, commonly expected |
| Heredoc `<<`, `<<-` | ✅ | |
| Herestring `<<<` | ✅ | bash extension |

### Builtins

| Builtin | Status | POSIX Section | Notes |
|---|---|---|---|
| `cd` | ✅ | 2.14.1 | No `-L`/`-P` flags yet |
| `pwd` | ✅ | 2.14.2 | |
| `echo` | ✅ | — | XSI extension; not fully portable per POSIX |
| `printf` | ✅ | 2.14.3 | Core format specifiers implemented |
| `export` | ✅ | 2.14.4 | Lists all vars when no args |
| `unset` | ✅ | 2.14.5 | |
| `exit` | ✅ | 2.14.6 | |
| `return` | ✅ | 2.14.7 | |
| `set` | ✅ | 2.14.8 | Partial: no full option parsing |
| `shift` | ✅ | 2.14.9 | |
| `test` / `[` | ✅ | 2.14.10 | File, string, numeric operators |
| `read` | ✅ | 2.14.11 | `-r` flag supported |
| `trap` | ✅ | 2.14.12 | Signal name → number mapping |
| `source` / `.` | ✅ | 2.14.13 | |
| `alias` | ✅ | XSI | Lists and defines aliases |
| `unalias` | ✅ | XSI | `-a` flag supported |
| `jobs` | ✅ | XSI | Job control extension |
| `fg` | ✅ | XSI | Job control extension |
| `bg` | ✅ | XSI | Job control extension |

### Missing POSIX Builtins

| Builtin | POSIX Section | Priority |
|---|---|---|
| `break` | 2.14.14 | High |
| `continue` | 2.14.15 | High |
| `eval` | 2.14.16 | High |
| `exec` | 2.14.17 | High |
| `getopts` | 2.14.18 | Medium |
| `hash` | 2.14.19 | Low |
| `readonly` | 2.14.20 | Medium |
| `times` | 2.14.21 | Low |
| `type` | 2.14.22 | Medium |
| `ulimit` | 2.14.23 | Low |
| `umask` | 2.14.24 | Medium |
| `wait` | 2.14.25 | High |
| `kill` | — | Medium |

### Job Control (XSI Extension)

| Feature | Status | Notes |
|---|---|---|
| Background execution `&` | ✅ | Fork, `setpgid(0, 0)`, immediate return |
| `jobs` builtin | ✅ | `-l`, `-p` flags; current/previous job markers |
| `fg` builtin | ✅ | Terminal control via `tcsetpgrp` |
| `bg` builtin | ✅ | `SIGCONT` via `killpg` |
| Process group management | ✅ | `setpgid`, `tcsetpgrp` |
| Signal handling | ✅ | `SIGCHLD` handler, `SIGINT`/`SIGQUIT` ignored in interactive |

### Special Parameters

| Parameter | Status | Notes |
|---|---|---|
| `$@`, `$*` | ✅ | Positional parameters |
| `$#` | ✅ | Number of positional parameters |
| `$?` | ✅ | Exit status |
| `$-` | ❌ | Current options flags |
| `$$` | ❌ | Process ID of shell |
| `$!` | ❌ | Process ID of last background job |
| `$0` | ❌ | Name of shell or script |

---

## Known Deviations

### 1. `break` / `continue` — Not Implemented

POSIX requires `break [n]` and `continue [n]` to exit/continue from `n` enclosing loops. These are parsed but not yet executed properly.

**Workaround:** None. Scripts using `break` or `continue` will fail.

### 2. `eval` — Not Implemented

POSIX requires `eval` to re-parse and execute its arguments as shell commands.

**Workaround:** None.

### 3. `exec` — Not Implemented

POSIX requires `exec [command [argument ...]]` to replace the shell with the command without forking.

**Workaround:** Use direct command execution; note that `exec` in redirection-only form (`exec 3>file`) is also not supported.

### 4. `wait` — Not Implemented

POSIX requires `wait [job]` to wait for background jobs. Critical for scripts that start background work and need synchronization.

**Workaround:** None.

### 5. `$-`, `$$`, `$!`, `$0` — Not Implemented

Special parameters beyond `$?`, `$#`, `$@`, `$*` are not yet supported.

**Workaround:** Avoid these in portable scripts.

### 6. Signal Specifier Differences

`trap` accepts signal names but does not yet support `trap - SIGNAL` to reset to default, nor `trap '' SIGNAL` to ignore in a standard way.

### 7. Word Splitting in Assignments

POSIX requires no word splitting in variable assignments (`var=value`). This is correctly implemented. However, the interaction between IFS and unquoted expansion in some edge cases may differ from bash/dash.

### 8. Exit Status of Pipelines

POSIX requires `set -o pipefail` behavior to be opt-in. Currently, modsh returns the exit status of the last command in a pipeline, which is the POSIX default without `pipefail`.

### 9. Here-document Tab Stripping

`<<-` (here-document with tab stripping) is implemented but tab stripping behavior may differ slightly from other shells regarding mixed tabs and spaces.

### 10. Function Exit via `return`

`return` from a function currently returns from the function context. However, `return` outside a function should be equivalent to `exit`, which may not yet be enforced.

### 11. Variable Assignment in Command Lists

Variable assignments before semicolons (`x=value; cmd`) may not be parsed correctly in all contexts. The assignment should persist in the current shell environment.

**Workaround:** Use separate commands or export the variable explicitly.

### 12. Arithmetic Expansion Edge Cases

Arithmetic expansion `$((expr))` works for simple expressions but may fail in complex contexts like within while loop conditions.

**Workaround:** Use `expr` external command for complex arithmetic.

### 13. Command Substitution in All Contexts

Command substitution `$(cmd)` works for simple cases but may not be recognized in all parser contexts.

**Workaround:** Assign to a variable first, then use the variable.

---

## Running the POSIX Test Suite

### Integration Tests

POSIX compliance tests live in `modsh-cli/tests/posix.rs`. They spawn the `modsh` binary and verify behavior against known POSIX test cases.

```bash
cargo test --package modsh-cli --test posix
```

**Current Status:** 19 tests passing, 10 known failures (documented above as deviations).

### Manual Testing

For one-off verification:

```bash
cargo build
./target/debug/modsh -c 'echo hello'
./target/debug/modsh -c 'test -f /etc/passwd && echo exists'
```

### External Test Suites

The following external suites are aspirational targets for future compliance work:

- **yash test suite** (GNU, shell test suite)
- **posh** (POSIX sh) test suite
- **dash** test suite
- **ksh93** compatibility tests

These are not yet integrated into CI due to the pre-alpha status of modsh.

---

## Compliance Roadmap

| Milestone | Target | Key Work |
|---|---|---|
| v0.1.0-alpha | Current | Core parser, executor, builtins, job control |
| v0.1.0-beta | Q3 2026 | `break`, `continue`, `eval`, `exec`, `wait`, special params `$!` `$$` |
| v0.1.0-rc | Q4 2026 | Full builtin set, `readonly`, `umask`, `type`, external POSIX test suite run |
| v1.0.0 | 2027 | Documented deviations < 5%, pass 90%+ of POSIX sh test suite |
