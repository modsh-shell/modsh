//! POSIX compliance integration tests
//!
//! Tests marked `#[ignore]` track known deviations documented in `POSIX.md`.
//! Run ignored tests with: `cargo test -- --ignored`

use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Spawn the `modsh` binary with `-c <cmd>` and capture (exit_code, stdout, stderr).
fn run(cmd: &str) -> (i32, String, String) {
    let modsh = std::env::var("CARGO_BIN_EXE_modsh")
        .expect("CARGO_BIN_EXE_modsh not set; run via `cargo test`");
    let child = Command::new(&modsh)
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn modsh ({modsh}): {e}"));
    let output = child.wait_with_output().expect("modsh wait failed");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(1);
    (code, stdout, stderr)
}

/// Assert exit code is 0, returning (stdout, stderr) for further checks.
#[track_caller]
fn assert_ok(cmd: &str) -> (String, String) {
    let (code, stdout, stderr) = run(cmd);
    assert_eq!(
        code, 0,
        "cmd: {cmd}\ncode: {code}\nstdout: {stdout}\nstderr: {stderr}"
    );
    (stdout, stderr)
}

// ---------- Basics ----------

#[test]
fn true_exits_0() {
    assert_eq!(run("true").0, 0);
}

#[test]
fn false_exits_1() {
    assert_eq!(run("false").0, 1);
}

#[test]
fn echo_basic() {
    let (out, _) = assert_ok("echo hello");
    assert_eq!(out.trim(), "hello");
}

#[test]
fn exit_42() {
    assert_eq!(run("exit 42").0, 42);
}

// ---------- Variables & parameters ----------

#[test]
#[ignore = "known deviation: variable assignment as first command in a list (POSIX.md #11)"]
fn var_assign_expand() {
    let (out, _) = assert_ok("x=hi; echo $x");
    assert_eq!(out.trim(), "hi");
}

#[test]
#[ignore = "known deviation: positional parameters $1, $2 not expanded in arg position (POSIX.md #5)"]
fn positional_params() {
    let (out, _) = assert_ok("set a b; echo $1 $2");
    assert_eq!(out.trim(), "a b");
}

#[test]
#[ignore = "known deviation: $# special parameter not yet expanded (POSIX.md #5)"]
fn dollar_hash() {
    let (out, _) = assert_ok("set a b; echo $#");
    assert_eq!(out.trim(), "2");
}

// ---------- Pipelines & redirections ----------

#[test]
fn pipeline() {
    let (out, _) = assert_ok("echo hello | cat");
    assert_eq!(out.trim(), "hello");
}

#[test]
fn redirect_out() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("out.txt");
    let p = path.display();
    let (out, _) = assert_ok(&format!("echo hi > {p}; cat {p}"));
    assert_eq!(out.trim(), "hi");
}

#[test]
fn redirect_in() {
    // Set up the file out-of-band so this test only exercises input redirect.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("in.txt");
    std::fs::write(&path, "hi\n").expect("write");
    let p = path.display();
    let (out, _) = assert_ok(&format!("cat < {p}"));
    assert_eq!(out.trim(), "hi");
}

#[test]
fn redirect_append() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("app.txt");
    let p = path.display();
    let (out, _) = assert_ok(&format!("echo a > {p}; echo b >> {p}; cat {p}"));
    assert_eq!(out.trim().lines().collect::<Vec<_>>(), vec!["a", "b"]);
}

// ---------- Control structures ----------

#[test]
fn if_true() {
    let (out, _) = assert_ok("if true; then echo yes; fi");
    assert_eq!(out.trim(), "yes");
}

#[test]
fn if_false() {
    let (out, _) = assert_ok("if false; then echo yes; else echo no; fi");
    assert_eq!(out.trim(), "no");
}

#[test]
#[ignore = "known deviation: variable expansion in for-loop body (POSIX.md #11)"]
fn for_loop() {
    let (out, _) = assert_ok("for x in a b c; do echo $x; done");
    assert_eq!(out.trim().lines().collect::<Vec<_>>(), vec!["a", "b", "c"]);
}

#[test]
#[ignore = "known deviation: arithmetic expansion inside while condition (POSIX.md #12)"]
fn while_loop() {
    let (out, _) = assert_ok("i=0; while test $i -lt 3; do echo $i; i=$((i+1)); done");
    assert_eq!(out.trim().lines().collect::<Vec<_>>(), vec!["0", "1", "2"]);
}

#[test]
fn case_stmt() {
    let (out, _) = assert_ok("case hello in hello) echo match;; esac");
    assert_eq!(out.trim(), "match");
}

// ---------- Lists ----------

#[test]
fn and_short_circuit() {
    let (code, stdout, _) = run("false && echo bad");
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
}

#[test]
fn or_executes() {
    let (out, _) = assert_ok("false || echo good");
    assert_eq!(out.trim(), "good");
}

// ---------- Subshell ----------

#[test]
#[ignore = "known deviation: variable assignment as first command in a list (POSIX.md #11)"]
fn subshell_isolation() {
    let (out, _) = assert_ok("x=outer; (x=inner; echo $x); echo $x");
    assert_eq!(
        out.trim().lines().collect::<Vec<_>>(),
        vec!["inner", "outer"]
    );
}

// ---------- Builtins ----------

#[test]
fn cd_builtin() {
    let dir = TempDir::new().expect("tempdir");
    // Canonicalize because /tmp may be a symlink to /private/tmp on macOS etc.
    let real = std::fs::canonicalize(dir.path()).expect("canonicalize");
    let p = real.display();
    let (out, _) = assert_ok(&format!("cd {p}; pwd"));
    assert_eq!(out.trim(), real.to_string_lossy());
}

#[test]
fn test_file_exists() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("f");
    std::fs::write(&path, b"").expect("write");
    let (out, _) = assert_ok(&format!("test -f {} && echo ok", path.display()));
    assert_eq!(out.trim(), "ok");
}

#[test]
fn test_numeric() {
    let (out, _) = assert_ok("test 1 -lt 2 && echo ok");
    assert_eq!(out.trim(), "ok");
}

#[test]
fn bracket_builtin() {
    let dir = TempDir::new().expect("tempdir");
    let (out, _) = assert_ok(&format!("[ -d {} ] && echo ok", dir.path().display()));
    assert_eq!(out.trim(), "ok");
}

#[test]
fn printf_builtin() {
    let (out, _) = assert_ok("printf 'hello %s\\n' world");
    assert_eq!(out, "hello world\n");
}

#[test]
fn alias_builtin() {
    // Accept any quoting style for the alias value per POSIX (implementation-defined).
    let (out, _) = assert_ok("alias ll='ls -l'; alias");
    assert!(
        out.contains("ll=") && out.contains("ls -l"),
        "expected alias listing to mention ll and 'ls -l', got: {out}"
    );
}

#[test]
fn function_call() {
    let (out, _) = assert_ok("f() { echo inside; }; f");
    assert_eq!(out.trim(), "inside");
}

// ---------- Expansions ----------

#[test]
#[ignore = "known deviation: arithmetic expansion in arg position (POSIX.md #12)"]
fn arith_expand() {
    let (out, _) = assert_ok("echo $((1+2))");
    assert_eq!(out.trim(), "3");
}

#[test]
#[ignore = "known deviation: command substitution in arg position (POSIX.md #13)"]
fn cmd_subst() {
    let (out, _) = assert_ok("echo $(echo hi)");
    assert_eq!(out.trim(), "hi");
}

#[test]
#[ignore = "known deviation: pathname (glob) expansion not active in arg position (POSIX.md)"]
fn glob_star() {
    // Build a tempdir with known files, then glob inside it.
    let dir = TempDir::new().expect("tempdir");
    for name in ["alpha.txt", "beta.txt"] {
        std::fs::write(dir.path().join(name), b"").expect("write");
    }
    let pattern = dir.path().join("*.txt");
    let (out, _) = assert_ok(&format!("echo {}", pattern.display()));
    assert!(out.contains("alpha.txt"), "expected glob match: {out}");
    assert!(out.contains("beta.txt"), "expected glob match: {out}");
}
