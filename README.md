# git-rename-commit

Rewrite a git commit message using a sed-style substitution.

When placed in `$PATH`, git auto-discovers it as `git rename-commit`.

## Usage

```
git rename-commit <commit> -e 's/pattern/replacement/flags'
git rename-commit <from>..<to> -e 's/pattern/replacement/flags'
git rename-commit -n <count> -e 's/pattern/replacement/flags'
```

**Sed flags:**
- `g` — replace all occurrences (default: first only)
- `i` — case-insensitive matching

The delimiter after `s` can be any character (e.g. `s|foo|bar|`).

## Examples

Rename the HEAD commit:

```bash
git rename-commit HEAD -e 's/old/new/'
```

Case-insensitive, global replace:

```bash
git rename-commit HEAD -e 's/hello/goodbye/gi'
```

Rewrite an older commit (descendants are rebuilt automatically):

```bash
git rename-commit abc123 -e 's/typo/fixed/'
```

Rename across the last 5 commits:

```bash
git rename-commit -n 5 -e 's/WIP/feat/'
```

Rename within a revision range (standard git `A..B` semantics — `A` excluded, `B` included):

```bash
git rename-commit abc123..def456 -e 's/old/new/'
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
