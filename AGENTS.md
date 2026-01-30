# AGENTS.md

## Project summary
- Build RustRLM: a Rust implementation of Recursive Language Models (RLM) that uses an embedded, lightweight, secure
  Python-REPL-compatible subset focused on string operations.
- The REPL engine is a Rust library crate; the RLM runner is a separate Rust crate/binary in the same workspace.

## Scope and guardrails
### Do
- Implement only the minimum Python syntax/features needed for string manipulation and RLM workflows.
- Prefer explicit allowlists for language features and stdlib access.
- Keep the interpreter deterministic and resource-bounded where possible.
- Write tests first (t_wada TDD style): red -> green -> refactor.

### Don't
- (REPL engine) Do not allow file I/O, networking, subprocesses, dynamic code loading, or eval/exec.
- Do not expose `__import__`, `open`, OS env access, or reflection hooks.
- Do not add non-essential features outside the RLM string-processing scope.

## Commands (Rust defaults)
- Build: `cargo build`
- Run: `cargo run`
- Test (all): `cargo test`
- Test (single): `cargo test <test_name>`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Format: `cargo fmt`

## Style and structure
- Follow `rustfmt` and `clippy` guidance; no unused imports or warnings.
- Naming: snake_case for functions/vars, CamelCase for types/traits, SCREAMING_SNAKE for consts.
- Prefer small modules, explicit types at public boundaries, and clear error enums.
### Communication
- Batch questions instead of asking them one-by-one.
- After the user answers the batch, proceed without additional confirmation unless a new blocker appears.

## Testing
- Add/adjust tests for every behavior change.
- Keep tests readable and close to the logic they cover when possible.
- If behavior is ambiguous, add a failing test and clarify before implementation.
- When adding or modifying runnable samples/examples, execute them and confirm they work before reporting completion.

## Security notes
- Treat all input as untrusted.
- Avoid panics on user input; return structured errors.
- Enforce limits (time, depth, size) if execution can grow unbounded.

## Continuity Ledger (compaction-safe)
Maintain a single Continuity Ledger for this workspace in `CONTINUITY.md`. The ledger is the canonical session briefing designed to survive context compaction; do not rely on earlier chat text unless it’s reflected in the ledger.

### How it works
- At the start of every assistant turn: read `CONTINUITY.md`, update it to reflect the latest goal/constraints/decisions/state, then proceed with the work.
- Update `CONTINUITY.md` again whenever any of these change: goal, constraints/assumptions, key decisions, progress state (Done/Now/Next), or important tool outcomes.
- Keep it short and stable: facts only, no transcripts. Prefer bullets. Mark uncertainty as `UNCONFIRMED` (never guess).
- If you notice missing recall or a compaction/summary event: refresh/rebuild the ledger from visible context, mark gaps `UNCONFIRMED`, ask up to 1–3 targeted questions, then continue.

### `functions.update_plan` vs the Ledger
- `functions.update_plan` is for short-term execution scaffolding while you work (a small 3–7 step plan with pending/in_progress/completed).
- `CONTINUITY.md` is for long-running continuity across compaction (the “what/why/current state”), not a step-by-step task list.
- Keep them consistent: when the plan or state changes, update the ledger at the intent/progress level (not every micro-step).

### In replies
- Begin with a brief “Ledger Snapshot” (Goal + Now/Next + Open Questions). Print the full ledger only when it materially changes or when the user asks.

### `CONTINUITY.md` format (keep headings)
- Goal (incl. success criteria):
- Constraints/Assumptions:
- Key decisions:
- State:
- Done:
- Now:
- Next:
- Open questions (UNCONFIRMED if needed):
- Working set (files/ids/commands):
