# selfwork — Build Status & Next Steps

**Last updated:** 2026-05-10
**Branch:** `selfwork-v1` on https://github.com/ematteson/codex
**Tracking spec:** [`selfworkDesignSpec.md`](./selfworkDesignSpec.md) (843 lines, v0.1)

---

## Where we are

Step 5 is implemented and pushed to `fork/selfwork-v1`. Codex CLI remains untouched (topology B - `selfwork` lives alongside `codex` in the same workspace).

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

**Targeted verification on 2026-05-10:**

- `cargo test -p selfwork-core`: 53 passing
- `cargo check -p selfwork-cli`: clean
- `cargo build -p selfwork-cli`: clean
- Live temp-workspace smoke: `selfwork init` seeded 37 directories and 29 files
- Live Reflect eval gate: `selfwork eval reflect --report` ran 4 cases and passed the 95% gate
- Live full eval gate: `selfwork eval all` ran 8 cases and passed Explore, Plan, and Reflect gates
- Live handoff smoke: `:switch explore -> reflect` wrote an `explore_to_reflect` migration and Explore session summary
- Live handoff smoke: `:switch reflect -> plan` wrote a fresh migration and started Plan

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
selfwork eval all       # live eval gate for Explore + Plan + Reflect starter cases
```

The CLI surface from spec §7 is scaffolded; unimplemented mode commands still print a structured "scaffolded but not implemented" message. `selfwork eval --new` is still scaffolded, not implemented.

## Decisions made

| Decision | Choice | Why |
|---|---|---|
| Binary topology | (b) `selfwork` alongside `codex` | Additive, reversible, both binaries share `codex-*` crates underneath |
| Workspace location | per-project (`<repo>/.selfwork/`) | Matches `.git` semantics |
| Branch | `selfwork-v1` off `explore-mode-and-notebook` | Existing branch's PR-readiness preserved |
| Commit cadence | per sub-piece | 18 implementation commits after the parent branch; latest Step 5 implementation commit is `173ae4ccf8` |
| Step 3 Codex embedding | (alpha) embed `codex-core` directly via `InProcessAppServerClient` | Highest-fidelity governance hooks; couples selfwork to codex-core's evolution |
| Session source | `SessionSource::Custom("selfwork")` | Avoids upstream enum churn while preserving source identity |
| Codex memory layer | disabled for selfwork sessions | Selfwork owns its own state model |
| Auth | reuse Codex auth/config | Keeps v1 simple; revisit only if separate identity becomes necessary |
| Filesystem ACLs | host-enforced permission profile per mode | Explore cannot write `shared/`; Plan can write `shared/commitments.md` only |
| Handoff format | full JSON Schema (typed Rust structs + `schemars` export) | Validate at runtime; spec §6.2 base + per-pair extensions |
| Eval scoring | deterministic text checks for Step 4 skeleton | Fast, inspectable, no judge-model dependency yet |

---

## Picking back up: Step 5.5 / Step 6 plan

Do not start Program mode before the runtime safety hooks are in place. The prompt has base crisis/dependency language, but §10 explicitly says these are runtime behaviors, not prose.

Suggested sub-piece breakdown:

**5.5a. Runtime risk detection skeleton**
- Add local classifiers for crisis/self-harm, exclusivity/dependency language, relapse/shame spiral, major-decision language, and long-session duration
- Store active flags in `state/risk_flags.json`
- Inject safety/dependency guidance into the active session when a trigger fires
- Add unit tests for classifier inputs and state writes

**5.5b. Gate evals on runtime hooks**
- Add eval cases that prove crisis override beats Explore, Plan, and Reflect shapes
- Add dependency-language eval cases that require human-support routing
- Re-run `selfwork eval all`

**Step 6. Program mode**
- Add `resources/modes/program/prompt.md`
- Keep `selfwork program` disabled until Program evals exist and pass at 100%
- Write the full Program eval suite across the 10 categories in §8.2, then iterate prompt/runtime until the gate passes

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
# Start with: read selfwork-STATUS.md, then begin runtime safety hooks.
```

The `selfwork` binary is at `codex-rs/target/debug/selfwork` after `cargo build -p selfwork-cli`. The release build of *Codex* (with the Explore-mode + notebook changes from the parent branch) is at `codex-rs/target/release/codex` and is what your `~/.local/bin/codex` symlink points to.
