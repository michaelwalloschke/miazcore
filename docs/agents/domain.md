# Domain Docs

How the engineering skills should consume this single-context repository's domain documentation.

## Before exploring, read these

- `CONTEXT.md` at the repository root
- Relevant architectural decisions under `docs/adr/`

If these files do not exist, proceed silently. The `/domain-modeling` skill creates them lazily when terms or decisions are resolved.

## File structure

```text
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-example-decision.md
│   └── 0002-another-decision.md
└── src/
```

## Use the glossary's vocabulary

When output names a domain concept, use the term defined in `CONTEXT.md`. Do not drift to synonyms the glossary explicitly avoids.

If the required concept is absent, reconsider whether the term belongs to the project or note the gap for `/domain-modeling`.

## Flag ADR conflicts

Surface contradictions with existing ADRs explicitly rather than silently overriding them.
