use anyhow::{bail, Context, Result};
use clap::Parser;
use git2::{Oid, Repository, Sort};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::process;

#[derive(Parser)]
#[command(about = "Rename a git commit message using a sed-style substitution")]
struct Cli {
    /// Commit hash (full or abbreviated)
    commit: String,

    /// Sed-style substitution, e.g. 's/foo/bar/g'
    #[arg(short, long = "expression")]
    e: String,
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sed = parse_sed_expression(&cli.e)?;

    let repo = Repository::discover(".").context("not a git repository")?;
    let target_oid = repo
        .revparse_single(&cli.commit)
        .with_context(|| format!("could not resolve '{}'", cli.commit))?
        .id();

    // Collect commits from HEAD down to (and including) the target
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TOPOLOGICAL)?;

    let mut commit_chain: Vec<Oid> = Vec::new();
    for oid_result in revwalk {
        let oid = oid_result?;
        commit_chain.push(oid);
        if oid == target_oid {
            break;
        }
    }

    if commit_chain.last() != Some(&target_oid) {
        bail!("commit {} is not an ancestor of HEAD", &cli.commit);
    }

    // Rewrite from target forward (reverse the chain)
    commit_chain.reverse();

    let mut oid_map: HashMap<Oid, Oid> = HashMap::new();
    let mut matched = false;

    for &old_oid in &commit_chain {
        let old_commit = repo.find_commit(old_oid)?;

        let mut message = old_commit.message().unwrap_or("").to_string();

        if old_oid == target_oid {
            let new_message = apply_sed(&sed, &message);
            if new_message != message {
                matched = true;
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

    if !matched {
        eprintln!("No changes made — pattern did not match the commit message.");
        process::exit(1);
    }

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
