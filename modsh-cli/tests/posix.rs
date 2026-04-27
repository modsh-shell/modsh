//! POSIX compliance integration tests
use std::process::{Command, Stdio};

fn run(cmd: &str) -> (i32, String, String) {
    let modsh =
        std::env::var("CARGO_BIN_EXE_modsh").unwrap_or_else(|_| "target/debug/modsh".to_string());
    let mut child = Command::new(&modsh)
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

fn assert_ok(cmd: &str) -> (String, String) {
    let (code, stdout, stderr) = run(cmd);
    assert_eq!(code, 0, "cmd: {cmd}\nstderr: {stderr}");
    (stdout, stderr)
}

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
#[test]
fn var_assign_expand() {
    let (out, _) = assert_ok("x=hi; echo $x");
    assert_eq!(out.trim(), "hi");
}
#[test]
fn positional_params() {
    let (out, _) = assert_ok("set a b; echo $1 $2");
    assert_eq!(out.trim(), "a b");
}
#[test]
fn dollar_hash() {
    let (out, _) = assert_ok("set a b; echo $#");
    assert_eq!(out.trim(), "2");
}
#[test]
fn pipeline() {
    let (out, _) = assert_ok("echo hello | cat");
    assert_eq!(out.trim(), "hello");
}
#[test]
fn redirect_out() {
    let (out, _) = assert_ok("echo hi > /tmp/m1.txt; cat /tmp/m1.txt");
    assert_eq!(out.trim(), "hi");
}
#[test]
fn redirect_in() {
    let (out, _) = assert_ok("echo hi > /tmp/m2.txt; cat < /tmp/m2.txt");
    assert_eq!(out.trim(), "hi");
}
#[test]
fn redirect_append() {
    let (out, _) = assert_ok("echo a > /tmp/m3.txt; echo b >> /tmp/m3.txt; cat /tmp/m3.txt");
    assert_eq!(out.trim().lines().collect::<Vec<_>>(), vec!["a", "b"]);
}
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
fn for_loop() {
    let (out, _) = assert_ok("for x in a b c; do echo $x; done");
    assert_eq!(out.trim().lines().collect::<Vec<_>>(), vec!["a", "b", "c"]);
}
#[test]
fn while_loop() {
    let (out, _) = assert_ok("i=0; while test $i -lt 3; do echo $i; i=$((i+1)); done");
    assert_eq!(out.trim().lines().collect::<Vec<_>>(), vec!["0", "1", "2"]);
}
#[test]
fn case_stmt() {
    let (out, _) = assert_ok("case hello in hello) echo match;; esac");
    assert_eq!(out.trim(), "match");
}
#[test]
fn and_short_circuit() {
    assert_eq!(run("false && echo bad").0, 1);
    assert!(run("false && echo bad").1.is_empty());
}
#[test]
fn or_executes() {
    let (out, _) = assert_ok("false || echo good");
    assert_eq!(out.trim(), "good");
}
#[test]
fn subshell_isolation() {
    let (out, _) = assert_ok("x=outer; (x=inner; echo $x); echo $x");
    assert_eq!(
        out.trim().lines().collect::<Vec<_>>(),
        vec!["inner", "outer"]
    );
}
#[test]
fn cd_builtin() {
    let (out, _) = assert_ok("cd /tmp; pwd");
    assert_eq!(out.trim(), "/tmp");
}
#[test]
fn test_file_exists() {
    let (out, _) = assert_ok("test -f /etc/passwd && echo ok");
    assert_eq!(out.trim(), "ok");
}
#[test]
fn test_numeric() {
    let (out, _) = assert_ok("test 1 -lt 2 && echo ok");
    assert_eq!(out.trim(), "ok");
}
#[test]
fn bracket_builtin() {
    let (out, _) = assert_ok("[ -d /tmp ] && echo ok");
    assert_eq!(out.trim(), "ok");
}
#[test]
fn printf_builtin() {
    let (out, _) = assert_ok("printf 'hello %s\\n' world");
    assert_eq!(out, "hello world\n");
}
#[test]
fn alias_builtin() {
    let (out, _) = assert_ok("alias ll='ls -l'; alias");
    assert!(out.contains("ll='ls -l'"), "{out}");
}
#[test]
fn function_call() {
    let (out, _) = assert_ok("f() { echo inside; }; f");
    assert_eq!(out.trim(), "inside");
}
#[test]
fn arith_expand() {
    let (out, _) = assert_ok("echo $((1+2))");
    assert_eq!(out.trim(), "3");
}
#[test]
fn cmd_subst() {
    let (out, _) = assert_ok("echo $(echo hi)");
    assert_eq!(out.trim(), "hi");
}
#[test]
fn glob_star() {
    let (out, _) = assert_ok("echo /bin/ech*");
    assert!(out.trim().contains("/bin/echo"), "{out}");
}
