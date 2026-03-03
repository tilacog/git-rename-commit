# git-rename-commit

Rewrite a git commit message using a sed-style substitution.

When placed in `$PATH`, git auto-discovers it as `git rename-commit`.

## Usage

```
git rename-commit 's/pattern/replacement/flags' <commit>
git rename-commit 's/pattern/replacement/flags' <from>..<to>
git rename-commit 's/pattern/replacement/flags' -n <count>
```

**Sed flags:**
- `g` — replace all occurrences (default: first only)
- `i` — case-insensitive matching

The delimiter after `s` can be any character (e.g. `s|foo|bar|`).

## Examples

Rename the HEAD commit:

```bash
git rename-commit 's/old/new/' HEAD
```

Case-insensitive, global replace:

```bash
git rename-commit 's/hello/goodbye/gi' HEAD
```

Rewrite an older commit (descendants are rebuilt automatically):

```bash
git rename-commit 's/typo/fixed/' abc123
```

Rename across the last 5 commits:

```bash
git rename-commit 's/WIP/feat/' -n 5
```

Rename within a revision range (standard git `A..B` semantics — `A` excluded, `B` included):

```bash
git rename-commit 's/old/new/' abc123..def456
```

Only commits whose messages actually match the pattern are rewritten. Non-matching commits keep their original message but may receive new OIDs if they are descendants of a rewritten commit.

## Exit codes

- `0` — at least one commit message was rewritten
- `1` — pattern did not match any commit message (no changes made)

## Install

```bash
cargo install --path .
```

## Build & test

```bash
cargo build
cargo test
```
