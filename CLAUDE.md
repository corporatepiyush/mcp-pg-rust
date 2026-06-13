# CLAUDE.md: Developer Responsibilities & Agent Behavior

## Homebrew Updates

**Responsibility**: Human developer

**What needs updating after each release**:
- Homebrew formula in `homebrew-mcp-postgres/Formula/mcp_postgres.rb`

**When**: After successful crates.io publication and GitHub release creation

**How**: Follow Step 4 in SKILLS.md (Update Package Managers section)
- Retrieve SHA256 from GitHub release tarball
- Update version placeholders v[VERSION] with actual release version
- Commit and push changes

**Agent behavior**: 
- I will NOT push to package manager repositories
- I will verify SKILLS.md procedures are followed before each release
- I will remind you to update Homebrew if missing from release checklist
- If you ask me to update it, I will first confirm you've provided the release artifacts (SHA256, version number)

## Document Authority

- **SKILLS.md**: Truth for SDLC procedures, decision trees, and acceptance criteria. Contains v[VERSION] placeholders for version-agnostic documentation.
- **CLAUDE.md**: This file. Clarifies what I (Claude Code) am responsible for vs. what requires human action.

All procedures in SKILLS.md should be respected and followed.
