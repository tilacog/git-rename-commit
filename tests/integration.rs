use std::process::Command;

fn binary_path() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_git-rename-commit").into()
}

/// Create a temporary git repo, returning its path.
/// The repo has user.name/user.email configured so commits work in CI.
fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["init"]);
    run(&["config", "user.name", "Test"]);
    run(&["config", "user.email", "test@test.com"]);
    dir
}

fn commit_empty(dir: &std::path::Path, message: &str) {
    let out = Command::new("git")
        .args(["commit", "--allow-empty", "-m", message])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn log_oneline(dir: &std::path::Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["log", "--format=%s"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

fn rev_parse(dir: &std::path::Path, rev: &str) -> String {
    let out = Command::new("git")
        .args(["rev-parse", rev])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn run_rename(dir: &std::path::Path, commit: &str, expr: &str) -> std::process::Output {
    Command::new(binary_path())
        .args([commit, "-e", expr])
        .current_dir(dir)
        .output()
        .unwrap()
}

#[test]
fn global_replacement() {
    let dir = init_repo();
    commit_empty(dir.path(), "Hello World Hello");

    let out = run_rename(dir.path(), "HEAD", "s/Hello/Bye/g");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    assert_eq!(log, vec!["Bye World Bye"]);
}

#[test]
fn case_insensitive() {
    let dir = init_repo();
    commit_empty(dir.path(), "UPPER case");

    let out = run_rename(dir.path(), "HEAD", "s/upper/lower/i");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    assert_eq!(log, vec!["lower case"]);
}

#[test]
fn no_match_exits_nonzero() {
    let dir = init_repo();
    commit_empty(dir.path(), "no match here");

    let out = run_rename(dir.path(), "HEAD", "s/xyz/abc/");
    assert!(!out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("No changes made"),
        "unexpected stderr: {stderr}"
    );

    // Message should be unchanged
    let log = log_oneline(dir.path());
    assert_eq!(log, vec!["no match here"]);
}

#[test]
fn rewrite_ancestor_rebuilds_descendants() {
    let dir = init_repo();
    commit_empty(dir.path(), "first commit");
    commit_empty(dir.path(), "second commit");
    commit_empty(dir.path(), "third commit");

    // Get the OID of the first commit
    let first_oid = rev_parse(dir.path(), "HEAD~2");

    let out = run_rename(dir.path(), &first_oid, "s/first/REWRITTEN/");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    assert_eq!(
        log,
        vec!["third commit", "second commit", "REWRITTEN commit"]
    );

    // All OIDs should have changed
    let new_first_oid = rev_parse(dir.path(), "HEAD~2");
    assert_ne!(first_oid, new_first_oid);
}

#[test]
fn non_global_replaces_only_first() {
    let dir = init_repo();
    commit_empty(dir.path(), "aaa bbb aaa");

    let out = run_rename(dir.path(), "HEAD", "s/aaa/zzz/");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    assert_eq!(log, vec!["zzz bbb aaa"]);
}
