---
name: llmtrack-release-commit
description: Prepara el release commit con changeset. Use when user says 'prepare release commit' or 'create release version' or 'hacer release'.
---

# llmtrack-release-commit

## Overview

Act as release manager for the llmtrack monorepo. This workflow prepares a new version release by: (1) analyzing git history since the last tag and verifying it against the tree at HEAD, and (2) creating a changeset file with the appropriate version bump. The user reviews and confirms each step: the skill suggests, the user decides.

The log is a starting point, not the source. Commit subjects in this repo describe the approach at the time of the commit, and a later commit in the same cycle routinely reverts, reverses or replaces it. The changeset this workflow writes becomes the release's changelog, read as a statement of fact about a shipped version, so it is built from the code at HEAD.

## Conventions

- Bare paths (e.g. `references/guide.md`) resolve from the skill root.
- `{skill-root}` resolves to this skill's installed directory.
- `{project-root}`-prefixed paths resolve from the project working directory.
- `{skill-name}` resolves to the skill directory's basename.

## On Activation

1. If `--headless` or `-H` flag is present, set `{headless_mode}=true` and skip all user confirmations — use the suggested bump and proceed.
2. Greet the user and confirm intent: "I'll prepare a new release. Let me start by analyzing what's changed since the last version."

## Stage 1: Analyze What Changed

1. List all git tags sorted by date (newest first): `git tag --sort=-v:refname`
2. Identify the most recent tag.
3. Run `git log <latest_tag>..HEAD --format="%H %an <%ae> %s"` and group commits by conventional commit type (feat, fix, docs, chore, refactor, test).
4. For each commit, extract the contributor's GitHub username:
   - For direct commits (non-merge), use the author name if it looks like a GitHub handle or the local part of their email before `@`.
   - For merge commits with format "Merge pull request #N from <user>/<branch>", extract `<user>` as the GitHub username.
   - Skip `github-actions[bot]` — no attribution needed.
   - The repo owner's commits (Abian, AbianS) don't need attribution — only external contributors.
5. **Classify the cycle before suggesting any bump.** Run `git diff --name-only <latest_tag>..HEAD` and look at what changed:
   - **Product cycle**: anything under `apps/server/`, `apps/dashboard/`, or the root build/CI files changed. Run every stage below as written.
   - **Price-list-only cycle**: nothing changed but `pricing/model_prices.json` and `apps/server/assets/model_prices.json.gz`, the daily commits of `prices.yml`. Running servers already sync from `pricing/model_prices.json`, so these alone do not need a release. Say so and stop, unless the user wants the new snapshot in the image.
   - **Nothing changed**: say so and stop.
6. **Verify the log against the tree before describing anything.** A commit
   message says what that commit intended. It is not evidence that the change
   survived to HEAD. Inside one cycle a fix gets reverted, a follow-up reverses
   an ordering, and a subject line describes an approach a later commit
   replaced. Every statement Stage 2 makes has to be checked against HEAD.
   - Surface the supersession candidates first:
     `git log <latest_tag>..HEAD --format="%s" --no-merges | sort | uniq -d`
     prints repeated subjects, which in this repo means one commit reworking
     another. Treat near-duplicate subjects in the same subsystem the same way.
   - Read the cumulative `git diff <latest_tag>..HEAD -- <path>` for a
     subsystem, not per-commit diffs. A per-commit diff shows what a commit
     did, not what is left.
   - Before writing down any claim, find it in the working tree: grep for the
     constant, function, route, migration or config key it names and read the
     surrounding code. If the symbol is missing or the code does the opposite,
     the log is stale and the tree wins.
   - Dependency commits need the manifest and the lockfile, never the subject
     alone. "Remove X" usually means "stop pulling X through path Y", and X
     often stays in the tree through another path.
   - A `feat:` type or a `BREAKING CHANGE` footer is a claim like any other.
     Confirm the behaviour is in HEAD before it drives the bump in step 8. If
     it was reverted or reworked, say so and adjust the suggestion.
7. Present a summary to the user:
   - Latest tag and date
   - Commit count since then
   - Breakdown by type
   - Notable changes (features, fixes, breaking changes)
   - The cycle classification from step 5, stated plainly
   - Anything the step 6 pass corrected, so the user sees where the log and the
     tree disagreed
8. **Suggest a single bump level** for the release based on the analysis:
   - `minor` if any commit message contains `BREAKING CHANGE` or `!:` after the type, or if there are `feat:` commits
   - `patch` if only `fix:`, `docs:`, `chore:`, `refactor:`, `test:` commits
   - Never suggest `major` while the product is on `0.x`. See "Versioning Policy" below.
9. **Interactive:** Ask the user to confirm or override the single release bump. Do not ask per package, the fixed group makes that irrelevant.
10. **Headless:** Use the suggested bump, skip confirmation.

### Versioning Policy

**Shipped artifacts are lockstep.** `@llmtrack/server` and `@llmtrack/dashboard` are declared as a `fixed` group in `.changeset/config.json`. They always share one version number and are bumped together even when a given package has no changes.

Consequences for this workflow:

- One changeset naming one of them bumps both. There is no per-package bump decision inside the group.
- The group takes the **highest** bump present, so a `major` anywhere drags the whole product to the next major.
- **While on `0.x`, never write a `major` changeset.** Use `minor` to signal a breaking change, per the standard 0.x convention. A `major` would push the product to 1.0.0 as a side effect.
- The version number identifies the llmtrack release, not the semver of an individual artifact. The changelog is what tells users which part actually changed.
- **Migrations.** From the first published release on, a schema change is a new migration in both `apps/server/migrations/sqlite` and `apps/server/migrations/postgres`, never an edit to a published one. If the diff edits an existing migration file, stop and tell the user before writing anything.

## Stage 2: Create Changeset

1. Generate a random changeset filename (e.g. `shiny-dogs-walk.md` — use a short memorable slug).
2. Create the file at `{project-root}/.changeset/<filename>.md` with format:

   ```markdown
   ---
   "<package-name>": "<bump>"
   ---

   <description of changes>
   ```

   Name only `"@llmtrack/server"` with the agreed bump. The fixed group propagates it to `@llmtrack/dashboard` automatically, so listing it is redundant and risks a mismatched bump level.

   The description should be a concise bullet-free summary of what changed, written for the changelog audience. For changes made by external contributors, append `(@github_username)` after the relevant change description.

   It describes the tree at HEAD, not the commit log. Anything step 6 of Stage 1 could not confirm in the code does not go in.

   Keep it scannable. The changelog is read by someone deciding whether to upgrade, so a cycle with sixty commits does not earn a longer entry than one with six, it earns harder editing.

3. **Interactive:** Show the generated changeset to the user and ask for approval before writing.
4. **Headless:** Write directly without confirmation.

## Finalize

1. Determine the new version string. Read the current version from `{project-root}/apps/server/Cargo.toml` and apply the agreed bump. Because of the fixed group this is the version for every shipped artifact in the release.
2. Suggest the release commit message. Derive it from the release version and title:
   - Format: `release: v<version> - <Release Title>`
   - Example: `release: v0.8.0 - Spend by Person`
   - Include the changeset and any files modified during the release workflow (including this skill) in the commit.

   `.github/workflows/release.yml` builds the GitHub release body from the `apps/server/CHANGELOG.md` section that `changeset version` generates from this changeset, and titles the release `llmtrack v<version>`.
3. Summarize what was created:
   - Changeset file path
   - The single version the whole fixed group moves to
   - Suggested commit message
4. Remind the user of next steps:
   - Review the changeset
   - Run `pnpm changeset version` to apply versions
   - Run `pnpm run build` to verify
   - Commit with: `git add .changeset/ .claude/skills/llmtrack-release-commit/ && git commit -m "release: v<version> - <Release Title>"`
   - Push
