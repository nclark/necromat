//! End-to-end tests of the `nm` command line: arguments, exit codes and
//! messages. The `nm eval` output itself is checked by `spec_cases.rs`.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn nm() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nm"))
}

fn run_eval(input: &[u8]) -> Output {
    let mut child = nm()
        .arg("eval")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nm eval");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait for nm eval")
}

#[test]
fn eval_empty_input_prints_nothing() {
    let out = run_eval(b"");
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
}

#[test]
fn eval_rejects_extra_arguments() {
    let out = nm().args(["eval", "x"]).output().expect("run nm");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn missing_directory_exits_1() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let out = nm()
        .current_dir(dir.path())
        .arg("nonexistent/dir/file.nm")
        .output()
        .expect("run nm");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "Error: directory does not exist: nonexistent/dir\n"
    );
}

#[test]
fn help_exits_0() {
    let out = nm().arg("--help").output().expect("run nm");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("nm eval"));
}
