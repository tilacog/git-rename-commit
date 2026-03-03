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

fn run_rename_last(dir: &std::path::Path, n: &str, expr: &str) -> std::process::Output {
    Command::new(binary_path())
        .args(["-n", n, "-e", expr])
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

// ----- Tests for -n / --last flag -----

#[test]
fn last_n_rewrites_all_matching() {
    let dir = init_repo();
    commit_empty(dir.path(), "foo first");
    commit_empty(dir.path(), "foo second");
    commit_empty(dir.path(), "foo third");

    let out = run_rename_last(dir.path(), "3", "s/foo/bar/");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    assert_eq!(log, vec!["bar third", "bar second", "bar first"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Rewrote 3 of 3 commits"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn last_n_rewrites_only_matching() {
    let dir = init_repo();
    commit_empty(dir.path(), "foo first");
    commit_empty(dir.path(), "no match here");
    commit_empty(dir.path(), "foo third");

    let out = run_rename_last(dir.path(), "3", "s/foo/bar/");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    assert_eq!(log, vec!["bar third", "no match here", "bar first"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Rewrote 2 of 3 commits"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn last_1_is_equivalent_to_head() {
    let dir = init_repo();
    commit_empty(dir.path(), "keep this");
    commit_empty(dir.path(), "foo latest");

    let out = run_rename_last(dir.path(), "1", "s/foo/bar/");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    assert_eq!(log, vec!["bar latest", "keep this"]);
}

#[test]
fn last_0_produces_error() {
    let dir = init_repo();
    commit_empty(dir.path(), "some commit");

    let out = run_rename_last(dir.path(), "0", "s/foo/bar/");
    assert!(!out.status.success());
}

#[test]
fn n_with_positional_commit_is_error() {
    let dir = init_repo();
    commit_empty(dir.path(), "some commit");

    let out = Command::new(binary_path())
        .args(["HEAD", "-n", "1", "-e", "s/foo/bar/"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn last_n_no_match_exits_nonzero() {
    let dir = init_repo();
    commit_empty(dir.path(), "alpha");
    commit_empty(dir.path(), "beta");
    commit_empty(dir.path(), "gamma");

    let out = run_rename_last(dir.path(), "3", "s/xyz/abc/");
    assert!(!out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("No changes made"),
        "unexpected stderr: {stderr}"
    );
}

// ----- Tests for revision range (A..B) syntax -----

#[test]
fn range_rewrites_only_commits_in_range() {
    let dir = init_repo();
    commit_empty(dir.path(), "foo first"); // HEAD~3
    commit_empty(dir.path(), "foo second"); // HEAD~2
    commit_empty(dir.path(), "foo third"); // HEAD~1
    commit_empty(dir.path(), "foo fourth"); // HEAD

    // Range: HEAD~3..HEAD~1 => includes HEAD~2 and HEAD~1, excludes HEAD~3 and HEAD
    let from = rev_parse(dir.path(), "HEAD~3");
    let to = rev_parse(dir.path(), "HEAD~1");
    let range = format!("{from}..{to}");

    let out = run_rename(dir.path(), &range, "s/foo/bar/");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    // HEAD and HEAD~3 should be unchanged, HEAD~1 and HEAD~2 should be rewritten
    assert_eq!(
        log,
        vec!["foo fourth", "bar third", "bar second", "foo first"]
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Rewrote 2 of 2 commits"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn range_no_match_exits_nonzero() {
    let dir = init_repo();
    commit_empty(dir.path(), "alpha");
    commit_empty(dir.path(), "beta");
    commit_empty(dir.path(), "gamma");

    let from = rev_parse(dir.path(), "HEAD~2");
    let to = rev_parse(dir.path(), "HEAD");
    let range = format!("{from}..{to}");

    let out = run_rename(dir.path(), &range, "s/xyz/abc/");
    assert!(!out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("No changes made"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn n_with_range_is_error() {
    let dir = init_repo();
    commit_empty(dir.path(), "first");
    commit_empty(dir.path(), "second");

    let from = rev_parse(dir.path(), "HEAD~1");
    let to = rev_parse(dir.path(), "HEAD");
    let range = format!("{from}..{to}");

    // -n and a range (passed as positional) should conflict
    let out = Command::new(binary_path())
        .args([&range, "-n", "1", "-e", "s/foo/bar/"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn range_excludes_from_commit() {
    let dir = init_repo();
    commit_empty(dir.path(), "foo first"); // HEAD~2 (this is <from>, should be excluded)
    commit_empty(dir.path(), "foo second"); // HEAD~1
    commit_empty(dir.path(), "foo third"); // HEAD

    // Range: HEAD~2..HEAD => includes HEAD~1 and HEAD, excludes HEAD~2
    let from = rev_parse(dir.path(), "HEAD~2");
    let range = format!("{from}..HEAD");

    let out = run_rename(dir.path(), &range, "s/foo/bar/");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log = log_oneline(dir.path());
    // HEAD~2 ("foo first") should remain unchanged
    assert_eq!(log, vec!["bar third", "bar second", "foo first"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Rewrote 2 of 2 commits"),
        "unexpected stderr: {stderr}"
    );
}
