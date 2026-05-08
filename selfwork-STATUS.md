# selfwork — Build Status & Next Steps

**Last updated:** 2026-05-08
**Branch:** `selfwork-v1` on https://github.com/ematteson/codex
**Tracking spec:** [`selfworkDesignSpec.md`](./selfworkDesignSpec.md) (843 lines, v0.1)

---

## Where we are

Step 4 is implemented and pushed to `fork/selfwork-v1`. Codex CLI remains untouched (topology B - `selfwork` lives alongside `codex` in the same workspace).

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

**Targeted verification on 2026-05-08:**

- `cargo test -p selfwork-core`: 51 passing
- `cargo check -p selfwork-cli`: clean
- `cargo build -p selfwork-cli`: clean
- Live temp-workspace smoke: `selfwork init` seeded 37 directories and 27 files
- Live eval gate: `selfwork eval all` ran 4 cases and passed both Explore and Plan gates

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
selfwork eval all       # live eval gate for Explore + Plan starter cases
```

The CLI surface from spec §7 is scaffolded; unimplemented mode commands still print a structured "scaffolded but not implemented" message. `selfwork eval --new` is still scaffolded, not implemented.

## Decisions made

| Decision | Choice | Why |
|---|---|---|
| Binary topology | (b) `selfwork` alongside `codex` | Additive, reversible, both binaries share `codex-*` crates underneath |
| Workspace location | per-project (`<repo>/.selfwork/`) | Matches `.git` semantics |
| Branch | `selfwork-v1` off `explore-mode-and-notebook` | Existing branch's PR-readiness preserved |
| Commit cadence | per sub-piece | 14 implementation commits after the parent branch; latest Step 4 implementation commit is `2b6be80636` |
| Step 3 Codex embedding | (alpha) embed `codex-core` directly via `InProcessAppServerClient` | Highest-fidelity governance hooks; couples selfwork to codex-core's evolution |
| Session source | `SessionSource::Custom("selfwork")` | Avoids upstream enum churn while preserving source identity |
| Codex memory layer | disabled for selfwork sessions | Selfwork owns its own state model |
| Auth | reuse Codex auth/config | Keeps v1 simple; revisit only if separate identity becomes necessary |
| Filesystem ACLs | host-enforced permission profile per mode | Explore cannot write `shared/`; Plan can write `shared/commitments.md` only |
| Handoff format | full JSON Schema (typed Rust structs + `schemars` export) | Validate at runtime; spec §6.2 base + per-pair extensions |
| Eval scoring | deterministic text checks for Step 4 skeleton | Fast, inspectable, no judge-model dependency yet |

---

## Picking back up: Step 5 plan

Step 5 is Reflect mode. Keep it smaller than Program: add the bundle, add Reflect evals at a 95% threshold, and make `selfwork reflect` runnable only when the starter eval gate is meaningful.

Suggested sub-piece breakdown:

**5a. Reflect bundle**
- Add `resources/modes/reflect/prompt.md`
- Flip `Mode::Reflect.is_implemented()` and `ModeBundle::for_mode`
- Wire `selfwork reflect` through `run_mode(Mode::Reflect, ...)`
- Seed Reflect prompt on `selfwork init`

**5b. Reflect eval starter suite**
- Add `resources/modes/reflect/evals.yaml`
- Include at least mode adherence, boundary adherence, crisis detection, and mode-boundary cases
- Update bootstrap eval seeding and tests
- Run `selfwork eval reflect` live before declaring the mode usable

**5c. Reflect handoff polish**
- Exercise `:switch explore` -> `reflect` and `reflect` -> `plan`
- Check that fresh-context entry uses the structured handoff without carrying the old tone
- Add targeted tests if compile-time coverage is not enough

Important unresolved item from the spec: dependency-monitoring runtime hooks in §10 are still not implemented. They probably deserve a dedicated Step 5.5 or Step 6 prerequisite before Program mode.

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
# Start with: read selfwork-STATUS.md, then begin Step 5 Reflect mode.
```

The `selfwork` binary is at `codex-rs/target/debug/selfwork` after `cargo build -p selfwork-cli`. The release build of *Codex* (with the Explore-mode + notebook changes from the parent branch) is at `codex-rs/target/release/codex` and is what your `~/.local/bin/codex` symlink points to.
