//! Specification cases: every `tests/spec/cases/<name>.nm` is fed to
//! `nm eval` on stdin, and stdout must equal `<name>.jsonl` byte for byte.
//!
//! `just spec-update` rewrites the `.jsonl` files from the current binary.
//! Review that diff before committing it: each changed line is a change in
//! nm's behaviour.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("spec")
        .join("cases")
}

/// The file stems in the cases directory with the given extension, sorted.
fn stems(ext: &str) -> Vec<String> {
    let dir = cases_dir();
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .map(|p| {
            p.file_stem()
                .expect("file stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    out.sort();
    out
}

fn run_eval(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nm"))
        .arg("eval")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn nm eval: {e}"))?;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .map_err(|e| format!("write stdin: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("wait for nm eval: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "exit {:?}, stderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

/// The first line where two outputs differ, for the failure message.
fn first_difference(got: &[u8], want: &[u8]) -> String {
    let got = String::from_utf8_lossy(got);
    let want = String::from_utf8_lossy(want);
    let mut got_lines = got.lines();
    let mut want_lines = want.lines();
    for n in 1.. {
        match (got_lines.next(), want_lines.next()) {
            (None, None) => break,
            (g, w) if g == w => {}
            (g, w) => {
                return format!(
                    "line {n}\n      got:  {}\n      want: {}",
                    g.unwrap_or("<missing>"),
                    w.unwrap_or("<missing>")
                );
            }
        }
    }
    "line endings or final newline".to_string()
}

#[test]
fn every_case_matches_its_golden() {
    let dir = cases_dir();
    let names = stems("nm");
    assert!(!names.is_empty(), "no cases in {}", dir.display());
    let mut failures = Vec::new();
    for name in &names {
        let input = std::fs::read(dir.join(format!("{name}.nm"))).expect("read case");
        let Ok(want) = std::fs::read(dir.join(format!("{name}.jsonl"))) else {
            continue; // reported by `every_case_has_a_golden`
        };
        match run_eval(&input) {
            Ok(got) if got == want => {}
            Ok(got) => failures.push(format!("{name}: {}", first_difference(&got, &want))),
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} spec cases differ (run `just spec-update` and review the diff \
         if the change is intended):\n  {}",
        failures.len(),
        names.len(),
        failures.join("\n  ")
    );
}

#[test]
fn every_case_has_a_golden() {
    let inputs = stems("nm");
    let goldens = stems("jsonl");
    let no_golden: Vec<&String> = inputs.iter().filter(|n| !goldens.contains(n)).collect();
    let no_input: Vec<&String> = goldens.iter().filter(|n| !inputs.contains(n)).collect();
    assert!(
        no_golden.is_empty() && no_input.is_empty(),
        ".nm without .jsonl: {no_golden:?}\n.jsonl without .nm: {no_input:?}"
    );
}
