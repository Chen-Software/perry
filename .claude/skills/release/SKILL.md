---
name: release
description: Create a new Perry release — commit all changes, push, bump version, tag, create GitHub release, build and install locally
disable-model-invocation: true
argument-hint: [description of changes]
allowed-tools: Bash, Read, Edit, Write, Glob, Grep
---

# New Perry Release

Create a new Perry release with all current uncommitted changes.

## Steps

1. **Survey changes**: Run `git status` and `git diff --stat` to understand all modified and new files. Read the diffs to understand what changed.

2. **Determine version**: Read `CLAUDE.md` for the current version. Bump the patch version (e.g., 0.4.6 → 0.4.7). If `$ARGUMENTS` mentions "minor" bump the minor version instead.

3. **Update version in Cargo.toml**: Edit `Cargo.toml` at the workspace level — the version is under `[workspace.package]`.

4. **Update CLAUDE.md**:
   - Update `**Current Version:**` to the new version
   - Add a new `### vX.Y.Z` section under `## Recent Changes` (above existing entries) with 1-2 line summaries of each change

5. **Stage and commit**: Stage all changed and new files by name (never use `git add -A`). Commit with message format:
   ```
   <type>: <concise summary> (vX.Y.Z)

   - bullet points for each change
   ```
   Where `<type>` is `fix`, `feat`, or `fix` + `feat` combined as appropriate.

6. **Push**: `git push origin main`

7. **Tag**: Create and push tag `vX.Y.Z`:
   ```
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```

8. **GitHub release**: Create via `gh release create` with a body containing:
   - `## Bug Fixes` section (if any fixes)
   - `## Features` section (if any features)
   - `## Tests` section (if new tests added)
   Each with bullet points describing the changes.

9. **Build all crates**: `cargo build --release` (timeout 10 minutes)

10. **Install perry binary**: `cargo install --path crates/perry --force` (timeout 10 minutes)

11. **Verify**: Run `perry --version` and confirm it shows the new version.

## Important

- Always read diffs before writing the commit message — understand what changed
- Never skip the build+install step — the release isn't done until perry is locally updated
- If the build fails, fix the issue before proceeding with the release
- Report the GitHub release URL and final `perry --version` output when done
