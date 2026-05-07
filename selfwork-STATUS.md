# selfwork — Build Status & Next Steps

**Last updated:** 2026-05-07
**Branch:** `selfwork-v1` on https://github.com/ematteson/codex
**Tracking spec:** [`selfworkDesignSpec.md`](./selfworkDesignSpec.md) (843 lines, v0.1)

---

## Where we are

Six commits on `fork/selfwork-v1`, all green, all pushed. Codex CLI untouched (topology B — selfwork lives alongside `codex` in the same workspace).

| Commit | Spec ref | What landed |
|---|---|---|
| `7af5cdf3b5` | §11 step 0 | `selfwork-cli` + `selfwork-core` crate scaffolding; full clap surface from §7; design spec committed at repo root |
| `64eddfef1c` | §11 step 1 | Base invariants + Explore mode prompt embedded; `ModeBundle::render_system_prompt` composes them in §2.2 order (invariants on top, mode prompt below) |
| `ea22b3aebb` | §11 step 2a | `SelfworkRoot` + `discover_root` (walks up from CWD looking for `.selfwork/`, like `.git`) |
| `753d4fdfca` | §11 step 2b | `selfwork init` — bootstraps the full §4 directory tree (36 dirs, 9 seeded files); idempotent |
| `1d1929a52d` | §11 step 2c | `selfwork journal today` — ensures today's entry, opens via `$EDITOR` |
| `78f8b91c76` | §11 step 2d | `selfwork status` — typed state files, prints workspace + active mode + session + risk flags |

**Test counts on the branch:**

- `selfwork-core`: 28 passing
- `codex-protocol`: 207 passing
- `codex-models-manager`: 27 passing
- `codex-notebook`: 16 passing
- `codex-analytics`: 48 passing

`cargo check --workspace` is clean. Codex's existing `Explore` mode + notebook (from the `explore-mode-and-notebook` branch, commit `a6f5fdd004`) is the parent of this branch and stays intact.

## What you can already do

```bash
# From any project directory
selfwork init           # creates .selfwork/ with full §4 layout
selfwork status         # prints workspace + active mode + risk flags
selfwork journal today  # creates/opens YYYY-MM-DD.md (uses $EDITOR if set)
selfwork prompt explore # prints the composed system prompt — invariants + Explore frame
```

The CLI surface from spec §7 is fully scaffolded; commands not yet implemented print a structured "scaffolded but not implemented" message instead of failing silently.

## Decisions made

| Decision | Choice | Why |
|---|---|---|
| Binary topology | (b) `selfwork` alongside `codex` | Additive, reversible, both binaries share `codex-*` crates underneath |
| Workspace location | per-project (`<repo>/.selfwork/`) | Matches `.git` semantics |
| Branch | `selfwork-v1` off `explore-mode-and-notebook` | Existing branch's PR-readiness preserved |
| Commit cadence | per sub-piece | 6 commits so far for steps 0-2 |
| Step 3 Codex embedding | (α) embed `codex-core` directly via `InProcessAppServerClient` | Highest-fidelity governance hooks; couples selfwork to codex-core's evolution |
| Step 3 scope | all three sub-steps before moving to Step 4 (evals) | session runner + handoff compiler + filesystem ACLs |
| Handoff format | full JSON Schema (typed Rust structs + `schemars` export) | Validate at runtime; spec §6.2 base + per-pair extensions |

## What didn't ship in Step 2

Spec §11 step 2 has four bullets; three landed. The fourth — *"Wire Explore's session output to write to `mode_private/explore/sessions/`"* — is deferred until Step 3 because it needs an actual session runner to call into. The disk write helper itself is trivial; what's missing is the session loop that calls it.

---

## Picking back up: Step 3 plan

Step 3 is the architecturally significant piece — it's where selfwork actually starts *running*. Sized realistically at 4-6 commits over a focused multi-hour stretch.

### Sub-piece breakdown

**3a. Session runner** (3-4 commits)
- 3a-i: dependency wiring — add `codex-core`, `codex-app-server-client`, `codex-arg0`, `codex-config`, `codex-feedback`, `codex-protocol` as `selfwork-core` deps; build `runtime::CodexRuntimeBuilder` that constructs the 16-field `InProcessClientStartArgs`. No session yet.
- 3a-ii: one-shot session runner — `run_one_shot(mode, user_message) -> assistant_text`. Single user message → single assistant response.
- 3a-iii: wire `selfwork explore "<prompt>"` to one-shot. First end-to-end smoke test against real Codex auth.
- 3a-iv: multi-turn — stdin/stdout chat loop wired to `selfwork explore` (no prompt arg).

**3b. Handoff compiler + `:switch` ceremony** (3-4 commits)
- 3b-i: handoff schema — typed Rust structs for `MigrationObject` and per-pair extensions (Explore→Reflect, Explore→Plan, Reflect→Program, Program→any). JSON Schema export via `schemars`. Files seeded under `.selfwork/modes/<mode>/handoff_in.schema.json` and `handoff_out.schema.json` at init time.
- 3b-ii: `:switch <mode>` parsing + handoff-out generation — outgoing mode populates the schema fields from session state; written to `.selfwork/state/migrations/<uuid>.json`.
- 3b-iii: fresh-context-on-entry — end the current Codex session, start a new one with the handoff JSON loaded as developer instructions for the incoming mode (spec §2.3 — "the previous transcript does not carry across the boundary").
- 3b-iv: write Explore session summary to `mode_private/explore/sessions/` on session end (the §11 step 2 deferred bullet).

**3c. Filesystem ACLs at the host** (2-3 commits)
- 3c-i: per-mode `manifest.yaml` — `read_paths`/`write_paths` declared per spec §5.1; loader reads them at session start.
- 3c-ii: plug masks into Codex's `codex-sandboxing` permission profile per session (spec §2.5: "enforced at the host, not in prose").
- 3c-iii: integration test — Explore session attempting to write to `shared/` is blocked by the sandbox, not just by prompt.

**Plan mode bundle** is the smallest bit and could land alongside 3a or 3b — just the prompt.md for the Plan frame and flipping `Mode::Plan.is_implemented` to true.

### Reference snippets to start 3a-i

The setup pattern lives in [`exec/src/lib.rs:506-525`](./codex-rs/exec/src/lib.rs). The client surface is [`app-server-client/src/lib.rs:331`](./codex-rs/app-server-client/src/lib.rs). Constructing `Config` is `Config::load_with_cli_overrides(vec![]).await` from [`core/src/config/mod.rs:1126`](./codex-rs/core/src/config/mod.rs).

Selfwork-specific config overrides we'll likely want:

- Disable Codex's collaboration modes (so they don't fight selfwork's mode system)
- Disable Codex's memories injection (selfwork has its own state model)
- Set `base_instructions` to our composed prompt (or set it to empty and inject ours via `developer_instructions` — needs experimentation)
- Add a `SessionSource::Selfwork` variant in `codex-protocol` (or reuse `SessionSource::Exec` initially)

### Open questions to resolve when resuming

1. **`SessionSource::Selfwork`** — add a new variant or reuse `Exec`? New variant is cleaner for telemetry and rollout filtering, but touches `codex-protocol` which means upstream-merge friction. Decide before 3a-i.

2. **What happens to Codex's collaboration-mode prompt + memories?** When selfwork sets up a session, do we strip those layers entirely (cleanest, but means selfwork sessions can't use memories) or keep them and accept some mixing? My lean: strip entirely for v1, revisit when the spec asks for cross-session memory.

3. **Auth.** Selfwork uses the same `~/.codex/auth.json` as Codex. Confirm this is desired vs. having a separate `~/.codex/selfwork-auth.json`. (Re-using is simpler; separating is cleaner if you want different ChatGPT logins for coding vs. personal-dev work.)

4. **Sandbox baseline.** Spec §4.1 has a per-mode access matrix. Selfwork's *baseline* sandbox (before per-mode tightening) — does it allow shell command execution at all? In Explore/Reflect/Program/Review it probably shouldn't (these aren't coding modes). Plan might need it for `git` operations. Decide the baseline before 3c-i.

5. **Dependency-monitoring runtime hooks.** Spec §10 specifies long-session, repeated-topic, and exclusivity-language triggers. These can't be model-side prose — they need runtime classifiers. Likely a separate sub-step within Step 3 or a standalone Step 3.5; flag during 3a-iv when we have a session loop to hook into.

---

## How to resume

```bash
# From any directory
cd /Users/UMATTER1/Documents/Source/Ways_of_Working/harness/codex
git switch selfwork-v1
git pull fork selfwork-v1   # if you've worked elsewhere

# Sanity check
cd codex-rs
cargo check --workspace
cargo test -p selfwork-core

# Pick up the build
# Start with: read selfwork-STATUS.md, then exec/src/lib.rs:506-525, then begin 3a-i.
```

The `selfwork` binary is at `codex-rs/target/debug/selfwork` after `cargo build -p selfwork-cli`. The release build of *Codex* (with the Explore-mode + notebook changes from the parent branch) is at `codex-rs/target/release/codex` and is what your `~/.local/bin/codex` symlink points to.
