use anyhow::{bail, Context, Result};
use clap::Parser;
use git2::{Oid, Repository, Sort};
use regex::{Regex, RegexBuilder};
use std::collections::{HashMap, HashSet};
use std::process;

#[derive(Parser)]
#[command(about = "Rename a git commit message using a sed-style substitution")]
struct Cli {
    /// Sed-style substitution, e.g. 's/foo/bar/g'
    expression: String,

    /// Commit hash (full or abbreviated), or a revision range (A..B)
    #[arg(required_unless_present = "n")]
    commit: Option<String>,

    /// Apply to the last N commits from HEAD
    #[arg(short, long = "last", conflicts_with = "commit")]
    n: Option<usize>,
}

struct SedExpr {
    pattern: Regex,
    replacement: String,
    global: bool,
}

fn parse_sed_expression(expr: &str) -> Result<SedExpr> {
    let bytes = expr.as_bytes();
    if bytes.first() != Some(&b's') {
        bail!("expression must start with 's': {expr}");
    }
    if bytes.len() < 2 {
        bail!("invalid sed expression: {expr}");
    }

    let delim = bytes[1] as char;
    let rest = &expr[2..];

    // Split on unescaped delimiters
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = rest.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next) = chars.peek() {
                if next == delim {
                    current.push(next);
                    chars.next();
                    continue;
                }
            }
            current.push(ch);
        } else if ch == delim {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    parts.push(current);

    if parts.len() < 2 {
        bail!("invalid sed expression: {expr}");
    }

    let pattern_str = &parts[0];
    let replacement = parts[1].clone();
    let flags_str = if parts.len() > 2 { &parts[2] } else { "" };

    let case_insensitive = flags_str.contains('i');
    let global = flags_str.contains('g');

    let pattern = RegexBuilder::new(pattern_str)
        .case_insensitive(case_insensitive)
        .build()
        .with_context(|| format!("invalid regex pattern: {pattern_str}"))?;

    Ok(SedExpr {
        pattern,
        replacement,
        global,
    })
}

fn apply_sed(sed: &SedExpr, input: &str) -> String {
    if sed.global {
        sed.pattern.replace_all(input, &sed.replacement).into()
    } else {
        sed.pattern.replace(input, &sed.replacement).into()
    }
}

fn resolve_targets(repo: &Repository, cli: &Cli) -> Result<(Vec<Oid>, HashSet<Oid>)> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TOPOLOGICAL)?;

    let mut commit_chain: Vec<Oid> = Vec::new();
    let mut target_set: HashSet<Oid> = HashSet::new();

    if let Some(n) = cli.n {
        if n == 0 {
            bail!("--last must be at least 1");
        }
        for oid_result in revwalk {
            let oid = oid_result?;
            commit_chain.push(oid);
            target_set.insert(oid);
            if commit_chain.len() == n {
                break;
            }
        }
    } else if let Some(ref commit_arg) = cli.commit {
        if let Some((from_str, to_str)) = commit_arg.split_once("..") {
            resolve_range(
                repo,
                commit_arg,
                from_str,
                to_str,
                revwalk,
                &mut commit_chain,
                &mut target_set,
            )?;
        } else {
            resolve_single(
                repo,
                commit_arg,
                revwalk,
                &mut commit_chain,
                &mut target_set,
            )?;
        }
    }

    Ok((commit_chain, target_set))
}

fn resolve_range(
    repo: &Repository,
    commit_arg: &str,
    from_str: &str,
    to_str: &str,
    revwalk: git2::Revwalk<'_>,
    commit_chain: &mut Vec<Oid>,
    target_set: &mut HashSet<Oid>,
) -> Result<()> {
    if from_str.is_empty() || to_str.is_empty() {
        bail!("both sides of the range must be specified: '{commit_arg}'");
    }

    let from_oid = repo
        .revparse_single(from_str)
        .with_context(|| format!("could not resolve '{from_str}'"))?
        .id();
    let to_oid = repo
        .revparse_single(to_str)
        .with_context(|| format!("could not resolve '{to_str}'"))?
        .id();

    // Normalize order so the range is direction-agnostic
    let (exclude, include) = if repo.graph_descendant_of(from_oid, to_oid)? {
        (to_oid, from_oid)
    } else {
        (from_oid, to_oid)
    };

    let mut range_walk = repo.revwalk()?;
    range_walk.push(include)?;
    range_walk.hide(exclude)?;
    range_walk.set_sorting(Sort::TOPOLOGICAL)?;

    for oid_result in range_walk {
        target_set.insert(oid_result?);
    }

    if target_set.is_empty() {
        bail!("no commits in range '{commit_arg}'");
    }

    // Build commit chain from HEAD down to the oldest target
    let mut remaining = target_set.len();
    for oid_result in revwalk {
        let oid = oid_result?;
        commit_chain.push(oid);
        if target_set.contains(&oid) {
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
    }

    if remaining > 0 {
        bail!("some commits in range '{commit_arg}' are not ancestors of HEAD");
    }

    Ok(())
}

fn resolve_single(
    repo: &Repository,
    commit_arg: &str,
    revwalk: git2::Revwalk<'_>,
    commit_chain: &mut Vec<Oid>,
    target_set: &mut HashSet<Oid>,
) -> Result<()> {
    let target_oid = repo
        .revparse_single(commit_arg)
        .with_context(|| format!("could not resolve '{commit_arg}'"))?
        .id();

    for oid_result in revwalk {
        let oid = oid_result?;
        commit_chain.push(oid);
        if oid == target_oid {
            break;
        }
    }

    if commit_chain.last() != Some(&target_oid) {
        bail!("commit {commit_arg} is not an ancestor of HEAD");
    }

    target_set.insert(target_oid);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sed = parse_sed_expression(&cli.expression)?;

    let repo = Repository::discover(".").context("not a git repository")?;
    let (mut commit_chain, target_set) = resolve_targets(&repo, &cli)?;

    let total_in_range = target_set.len();

    // Rewrite from oldest to newest
    commit_chain.reverse();

    let mut oid_map: HashMap<Oid, Oid> = HashMap::new();
    let mut rewrite_count: usize = 0;

    for &old_oid in &commit_chain {
        let old_commit = repo.find_commit(old_oid)?;
        let mut message = old_commit.message().unwrap_or("").to_string();

        if target_set.contains(&old_oid) {
            let new_message = apply_sed(&sed, &message);
            if new_message != message {
                rewrite_count += 1;
                eprintln!("Rewriting {}:", &old_oid.to_string()[..12]);
                eprintln!("  - {}", message.trim_end());
                eprintln!("  + {}", new_message.trim_end());
                message = new_message;
            }
        }

        // Remap parents
        let new_parents: Vec<git2::Commit> = old_commit
            .parent_ids()
            .map(|pid| {
                let mapped = oid_map.get(&pid).copied().unwrap_or(pid);
                repo.find_commit(mapped).unwrap()
            })
            .collect();
        let parent_refs: Vec<&git2::Commit> = new_parents.iter().collect();

        let new_oid = repo.commit(
            None,
            &old_commit.author(),
            &old_commit.committer(),
            &message,
            &old_commit.tree()?,
            &parent_refs,
        )?;

        oid_map.insert(old_oid, new_oid);
    }

    if rewrite_count == 0 {
        eprintln!(
            "No changes made \u{2014} pattern did not match any commit message in the range."
        );
        process::exit(1);
    }

    eprintln!("Rewrote {rewrite_count} of {total_in_range} commits.");

    // Update the current branch to point at the new HEAD
    let head_ref = repo.head()?;
    let new_head = oid_map[commit_chain.last().unwrap()];

    if head_ref.is_branch() {
        let branch_name = head_ref.name().unwrap();
        repo.reference(branch_name, new_head, true, "git-rename-commit")?;
    } else {
        repo.set_head_detached(new_head)?;
    }

    Ok(())
}
