# Issue tracker: Beads

Issues and specs for this repo live in beads, managed with the `bd` CLI (already initialised in `.beads/`).

## Conventions

- One feature = one epic bead. The spec goes in its description (or `--design`).
- Implementation tickets are child beads of the epic: `bd create "<title>" --parent <epic-id>`.
- Blocking edges: `bd create ... --deps blocks:<id>` or `bd dep add <ticket> --blocked-by <blocker>`. A ticket is unblocked when `bd ready` lists it.
- Comments and history: `bd comments add <id> "<text>"` (or `--append-notes`).
- Acceptance criteria go in `--acceptance`.

## When a skill says "publish to the issue tracker"

`bd create "<title>" -d "<body>"` (use `--body-file -` for long bodies, `--parent` for tickets under an epic).

## When a skill says "fetch the relevant ticket"

`bd show <id>`. The user will normally pass the bead id.

## Wayfinding operations

Used by `/wayfinder`.

- **Map**: an epic bead holding the Notes / Decisions-so-far / Fog body.
- **Child ticket**: a child bead (`--parent <map-id>`) with the question in the body and a `type:<research|prototype|grilling|task>` label.
- **Blocking**: `bd dep add <ticket> --blocked-by <blocker>`.
- **Frontier**: `bd ready --parent <map-id>`; first by id wins.
- **Claim**: `bd update <id> --claim` before any work.
- **Resolve**: record the answer with `bd comments add <id> "<answer>"`, then `bd close <id>`, then append a context pointer (gist + id) to the map's Decisions-so-far.
