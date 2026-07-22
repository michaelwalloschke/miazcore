# Issue tracker: Local Markdown

Issues and PRDs for this repo live as markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The PRD is `.scratch/<feature-slug>/PRD.md`
- Implementation issues are `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`
- Triage state is recorded as a `Status:` line near the top of each issue file
- Comments and conversation history append under a `## Comments` heading

## Wayfinding operations

- Map: `.scratch/<effort>/map.md`
- Child ticket: `.scratch/<effort>/issues/NN-<slug>.md`
- Ticket metadata uses `Type:`, `Status:`, and optional `Blocked by:` lines
- The frontier consists of open, unblocked, unclaimed tickets; lowest number wins
- Claim by setting `Status: claimed` before beginning work
- Resolve by adding `## Answer`, setting `Status: resolved`, and linking its gist from the map
