---
description: Update Lightfriend documentation and create commit
agent: general-purpose
---

You are tasked with updating Lightfriend project documentation before committing changes.

## Steps to follow:

1. **Review recent changes**
   - Use `git diff --name-only` to see changed files
   - Use `git diff` to see detailed changes
   - Use `git status` to see current state

2. **Identify documentation that needs updating**
   - Check if changes affect **README.md** (project overview, quick start)
   - Check if changes affect **CLAUDE.md** (development guide for AI assistants)
   - Check if changes affect **docs/DOCKER_SETUP.md** (Docker deployment instructions)
   - Check if changes affect **docs/MATRIX_SETUP_GUIDE.md** (Matrix bridge configuration)
   - Check if backend changes require updating API documentation
   - Check if frontend changes affect user-facing features
   - **Always update docs/CHANGELOG.md** with new features, changes, or fixes

3. **Update documentation based on changes**

   **For backend changes:**
   - Update CLAUDE.md if new patterns, integrations, or tools added
   - Update docs/DOCKER_SETUP.md if Docker config or deployment changed
   - Update docs/MATRIX_SETUP_GUIDE.md if Matrix bridge setup changed
   - Add entry to docs/CHANGELOG.md under appropriate category (Added/Changed/Fixed)

   **For frontend changes:**
   - Update README.md if user interface changed
   - Update CLAUDE.md if new frontend pages or components added
   - Add entry to docs/CHANGELOG.md under appropriate category

   **For database changes:**
   - Update CLAUDE.md if new tables or migration patterns introduced

   **For infrastructure changes:**
   - Update docs/DOCKER_SETUP.md for Docker/compose changes
   - Update deployment instructions if needed
   - Add entry to docs/CHANGELOG.md under appropriate category

4. **Create commit**
   - Stage all changes (code + documentation updates)
   - Create commit with conventional commit format:
     - `docs: update [doc-name] for [feature/change]` - if only docs changed
     - `feat: [feature description]` - if feature + docs (include doc updates in description)
     - `fix: [fix description]` - if fix + docs
     - `refactor: [description]` - if refactoring + docs
   - **Do NOT add "Generated with Claude Code" or Co-Authored-By lines** (per CLAUDE.md guidelines)

5. **Summary**
   - Show what documentation was updated and why
   - Show the commit message that was created
   - Confirm all changes are committed with `git status`

## Lightfriend-Specific Guidelines:

**CLAUDE.md updates:**
- Keep it concise (currently ~97 lines, avoid bloat)
- Update Architecture section if backend/frontend structure changed
- Update Key Patterns if new conventions introduced
- Update Common Tasks if referencing new skills

**docs/DOCKER_SETUP.md updates:**
- Update if bridge configurations changed
- Update if environment variables added/changed
- Update if justfile commands modified

**docs/CHANGELOG.md updates:**
- Add entry under [Unreleased] section
- Use appropriate category: Added, Changed, Fixed, Security, etc.
- Be concise but descriptive (one line per change)
- Group related changes together

**README.md updates:**
- Keep user-focused (not developer-focused)
- Update if deployment process changed
- Update if project scope expanded

**Style guidelines:**
- Follow existing documentation tone
- Be concise but complete
- Use code blocks for commands and file paths
- Keep line lengths reasonable for readability
- Use bullet points for lists

## Important:
- Review git commits guide in CLAUDE.md:82-85 (no AI attribution)
- Don't create a commit if no documentation actually needs updating
- Ensure documentation changes are in the same commit as related code changes
