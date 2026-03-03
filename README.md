# git-rename-commit

Rewrite a git commit message using a sed-style substitution.

When placed in `$PATH`, git auto-discovers it as `git rename-commit`.

## Usage

```
git rename-commit <commit> -e 's/pattern/replacement/flags'
```

**Flags:**
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

## Exit codes

- `0` — commit message was rewritten
- `1` — pattern did not match (no changes made)

## Install

```bash
cargo install --path .
```

## Build & test

```bash
cargo build
cargo test
```
