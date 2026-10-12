# AGENTS.md

## Code Style

- All code comments must be written in English.
- Commit messages must follow [Conventional Commits](https://www.conventionalcommits.org/) specification:
    - Format: `<type>(<scope>): <description>`
    - Common types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `build`
    - Example: `feat(core): add zip utilities for archive operations`
- All commits must be signed-off using the `-s` flag (`git commit -s`).
- When listing parallel items with no specific logical relationship, sort them alphabetically.

## Implementation Guidance

- Implement features elegantly and with extensibility in mind. Split code into focused files/modules when that keeps the
  design clearer, reduces coupling, or makes future feature types easier to add.
- When a task explicitly calls for refactoring or migration, treat the target architecture as the source of truth rather
  than preserving the old structure by default. Refactor boldly: move responsibilities to the right layer, delete
  redundant code, split or merge modules where it clarifies ownership, and leave compatibility shims only when they are
  necessary for a staged transition.
- When the user explicitly asks not to preserve compatibility aliases during a rename or migration, remove the old names
  across code, docs, command surfaces, and tests instead of keeping transitional aliases.
