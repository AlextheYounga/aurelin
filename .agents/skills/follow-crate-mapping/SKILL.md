---
name: follow-crate-mapping
description: Enforce Aurelin's researched crate choices when selecting dependencies, designing architecture, implementing Laravel-equivalent capabilities, changing generated starters, or reviewing whether code uses the intended Rust stack. Use whenever work adds, replaces, bypasses, or directly uses a dependency covered by docs/crate-mapping.md.
---

# Follow Aurelin's Crate Mapping

## Start with the project decisions

- Read the relevant parts of `CONTEXT.md` and `docs/crate-mapping.md` before choosing or using a dependency.
- Match behavior against the Laravel version named in `CONTEXT.md`.
- Find the capability's row in the mapping and verify its bold leading candidate first.
- Check the mapping's integration notes and leading-candidates section; a dependency named as an underlying driver does not replace the selected higher-level component.

## Follow the mapped stack

- Use the leading candidate when it satisfies the required behavior and project constraints.
- Preserve distinct layers identified by the mapping. For example, use SeaORM for application data access and migrations while allowing SQLx where it is SeaORM's driver or an explicitly mapped infrastructure dependency such as a session store.
- Do not silently use a lower-level crate merely because it is already transitive, familiar, or quicker for a small implementation.
- Do not add both a leading candidate and its alternative for the same role.
- Keep generated applications and starter templates aligned with the same choices as framework code.

## Verify before adoption

- Confirm the current crate version, feature flags, MSRV, license, maintenance status, and compatibility with the rest of Aurelin's stack.
- Verify actual APIs and runtime behavior from authoritative documentation or source.
- Treat the mapping as a researched default, not proof that an integration works.

## Handle deviations explicitly

- Use a mapped alternative only when the leading candidate has a verified incompatibility or cannot provide required behavior.
- Record the concrete gap and the resulting trade-off before changing direction.
- Ask for explicit human approval before adopting an alternative or an unmapped dependency for a mapped role.
- Update `docs/crate-mapping.md` when an approved decision supersedes its research or materially changes its integration notes.

## Review existing work

- When existing code conflicts with the mapping, stop extending the conflicting approach.
- Identify whether the apparent conflict is a legitimate lower-level or infrastructure use.
- Correct an actual mismatch before building more behavior on it, preserving observable behavior with tests.
- Report application-level choices precisely; do not describe an underlying driver as the selected ORM, cache abstraction, queue system, or other higher-level component.
