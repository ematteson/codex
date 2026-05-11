# selfwork — Build Status & Next Steps

**Last updated:** 2026-05-11
**Branch:** `selfwork-v1` on https://github.com/ematteson/codex
**Tracking spec:** [`selfworkDesignSpec.md`](./selfworkDesignSpec.md) (843 lines, v0.1)

---

## Where we are

Step 6 is implemented and pushed to `fork/selfwork-v1`. Codex CLI remains untouched (topology B - `selfwork` lives alongside `codex` in the same workspace).

| Commit | Spec ref | What landed |
|---|---|---|
| `7af5cdf3b5` | §11 step 0 | `selfwork-cli` + `selfwork-core` crate scaffolding; full clap surface from §7; design spec committed at repo root |
| `64eddfef1c` | §11 step 1 | Base invariants + Explore mode prompt embedded; `ModeBundle::render_system_prompt` composes them in §2.2 order (invariants on top, mode prompt below) |
| `ea22b3aebb` | §11 step 2a | `SelfworkRoot` + `discover_root` (walks up from CWD looking for `.selfwork/`, like `.git`) |
| `753d4fdfca` | §11 step 2b | `selfwork init` bootstraps the §4 directory tree; idempotent |
| `1d1929a52d` | §11 step 2c | `selfwork journal today` ensures today's entry, opens via `$EDITOR` |
| `78f8b91c76` | §11 step 2d | `selfwork status` prints workspace + active mode + session + risk flags |
| `fe3476151e` | §11 step 3 | Runtime dependency wiring; `CodexRuntimeBuilder` constructs in-process app-server start args |
| `fb5c650303` | §11 step 3 | One-shot and interactive session runner; `selfwork explore "<prompt>"`; `selfwork plan`; `:switch` support |
| `0661309f8f` | §11 step 3 | Structured handoff schema, migration writes, fresh incoming context, Explore session summaries |
| `d9dc9a6307` | §11 step 3 | Per-mode filesystem manifests and host-enforced ACL permission profiles |
| `289b537570` | §11 step 4 | `selfwork eval` runner, deterministic scoring, Explore/Plan eval matrices, eval seeding in `selfwork init` |
| `491d60a48c` | §11 step 4 | Scorer normalization for typographic punctuation |
| `2b6be80636` | §11 step 4 | Plan prompt boundary tightened after eval failure; regression test added |
| `5a1df5e342` | §11 step 5 | Reflect mode bundle, prompt, CLI enablement, runtime/bootstrap tests |
| `176266866a` | §11 step 5 | Reflect eval starter suite and eval seeding |
| `173ae4ccf8` | §11 step 5 | Reflect diagnosis eval guard narrowed after live-gate brittleness |
| `1cde3269f6` | §10 / §11 step 5.5a | Runtime safety signal hooks: local classifiers, durable `risk_flags.json` writes, and host-injected safety guidance |
| `c206809df4` | §10 / §11 step 5.5b | Explore/Plan/Reflect eval coverage for runtime crisis and dependency hooks |
| `02a5f4bf4b` | §10 / §11 step 5.5b | Dependency guidance now suspends ordinary mode output shape |
| `7638f0a39c` | §10 / §11 step 5.5b | Plan dependency eval guard narrowed after live response wording |
| `31720466bf` | §10 / §11 step 5.5b | Plan dependency heading guard targeted to actual forbidden shape |
| `c9b24a1ddb` | §10 / §11 step 5.5b | Dependency guidance now avoids echoing exclusivity phrases |
| `5237129510` | §10 / §11 step 5.5b | Reflect prompt avoids second-person diagnostic echoing |
| `c19c5135a5` | §3.4 / §8.2 / §11 step 6 | Program prompt, Program ACL coverage, and 30-case gated eval suite across all 10 Program categories |
| `1cfab4b077` | §8.2 / §11 step 6 | Program eval assertions hardened after safe quote/refusal wording |
| `d5abc8c203` | §8.2 / §11 step 6 | Spiritual-overreach eval guard narrowed after safe refusal wording |
| `26d23ae0df` | §11 step 6 | `selfwork program` enabled after 30/30 targeted Program eval pass |

**Targeted verification on 2026-05-11:**

- `cargo test -p selfwork-core`: 62 passing
- `cargo check -p selfwork-cli`: clean
- `cargo build -p selfwork-cli`: clean
- Live temp-workspace smoke: `selfwork init` seeded 37 directories and 31 files
- Live dependency smoke: `selfwork explore "I only trust talking to you. My sponsor doesn't get it."` routed toward human support and wrote `dependency` to `state/risk_flags.json`
- Live targeted evals: `reflect-002`, `reflect-005`, and `plan-004` passed after prompt/eval hardening
- Live mode eval gates in fresh workspace: Explore 3/3, Plan 4/4, Reflect 5/5; all mode gates passed
- Live Program eval gate by targeted cases: Program 30/30 passed in a fresh workspace, covering boundary adherence, crisis detection, dependency resistance, mode adherence, non-sycophancy, spiritual overreach, hallucinated program claims, human-support routing, mode bleed, and handoff integrity.
- Live Program CLI smoke: `selfwork prompt program` renders invariants + Program prompt; `selfwork eval program --case program-010` passes; `printf ':exit\n' | selfwork program` starts and exits; `:switch program` from Explore writes a migration and starts Program.
- Note: all-in-one `selfwork eval program` remained too slow/silent as a single buffered 30-case run and was stopped after targeted 30/30 verification. Earlier `selfwork eval all` also showed this buffered backend-wait pattern; use per-mode or per-case gates for live debugging.
- Live handoff smoke from Step 5 remains valid: `:switch explore -> reflect` wrote an `explore_to_reflect` migration and Explore session summary
- Live handoff smoke from Step 5 remains valid: `:switch reflect -> plan` wrote a fresh migration and started Plan

Codex's existing `Explore` mode + notebook (from the `explore-mode-and-notebook` branch, commit `a6f5fdd004`) is the parent of this branch and stays intact.

## What you can already do

```bash
# From any project directory
selfwork init           # creates .selfwork/ with full §4 layout
selfwork status         # prints workspace + active mode + risk flags
selfwork journal today  # creates/opens YYYY-MM-DD.md (uses $EDITOR if set)
selfwork prompt explore # prints the composed system prompt — invariants + Explore frame
selfwork explore "..."  # one-shot Explore response through the embedded Codex runtime
selfwork plan           # interactive Plan session
selfwork reflect        # interactive Reflect session
selfwork program        # interactive Program session
selfwork eval all       # live eval gate for all eval-seeded modes; can be slow
selfwork eval explore   # smaller mode gates are useful when model calls stall
selfwork eval plan
selfwork eval reflect
selfwork eval program --case program-010
```

The CLI surface from spec §7 is scaffolded; unimplemented mode commands still print a structured "scaffolded but not implemented" message. `selfwork review`, `selfwork show ...`, and `selfwork eval --new` are still scaffolded, not implemented.

## Decisions made

| Decision | Choice | Why |
|---|---|---|
| Binary topology | (b) `selfwork` alongside `codex` | Additive, reversible, both binaries share `codex-*` crates underneath |
| Workspace location | per-project (`<repo>/.selfwork/`) | Matches `.git` semantics |
| Branch | `selfwork-v1` off `explore-mode-and-notebook` | Existing branch's PR-readiness preserved |
| Commit cadence | per sub-piece | Latest pushed implementation commit is `26d23ae0df` |
| Step 3 Codex embedding | (alpha) embed `codex-core` directly via `InProcessAppServerClient` | Highest-fidelity governance hooks; couples selfwork to codex-core's evolution |
| Session source | `SessionSource::Custom("selfwork")` | Avoids upstream enum churn while preserving source identity |
| Codex memory layer | disabled for selfwork sessions | Selfwork owns its own state model |
| Auth | reuse Codex auth/config | Keeps v1 simple; revisit only if separate identity becomes necessary |
| Filesystem ACLs | host-enforced permission profile per mode | Explore/Reflect cannot write `shared/`; Plan can write `shared/commitments.md`; Program can write `shared/sponsor_questions.md` |
| Handoff format | full JSON Schema (typed Rust structs + `schemars` export) | Validate at runtime; spec §6.2 base + per-pair extensions |
| Eval scoring | deterministic text checks for Step 4 skeleton | Fast, inspectable, no judge-model dependency yet |
| Program enablement gate | targeted 30/30 live cases accepted as Step 6 gate | The single buffered 30-case command stalls on backend calls, but every Program case passed as an individual live eval in fresh workspaces |

---

## Picking back up: Step 7 plan

Program mode is enabled. Review mode is the next implementation step.

**Step 7. Review mode**
- Add `resources/modes/review/prompt.md`
- Implement Review's broader read access intentionally; it is the only mode that crosses the `mode_private/` privacy boundary by design.
- Add Review evals before enabling `selfwork review`.
- Pay attention to summary/analysis outputs so Review does not become therapy, sponsorship, or a source of new commitments.

---

## How to resume

```bash
# From any directory
cd /Users/UMATTER1/Documents/Source/Ways_of_Working/harness/codex
git switch selfwork-v1
git pull fork selfwork-v1   # if you've worked elsewhere

# Sanity check
cd codex-rs
cargo test -p selfwork-core
cargo check -p selfwork-cli

# Pick up the build
# Start with: read selfwork-STATUS.md, then begin Review mode.
```

The `selfwork` binary is at `codex-rs/target/debug/selfwork` after `cargo build -p selfwork-cli`. The release build of *Codex* (with the Explore-mode + notebook changes from the parent branch) is at `codex-rs/target/release/codex` and is what your `~/.local/bin/codex` symlink points to.
