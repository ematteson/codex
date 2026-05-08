//! Selfwork CLI entry point.
//!
//! v1 surface (spec §7). Step 0: stub commands so `--help` documents the
//! shape we are building toward; subsequent migration steps fill them in.

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use clap::Subcommand;
use std::io::Write;

#[derive(Debug, Parser)]
#[command(
    name = "selfwork",
    version = selfwork_core::SELFWORK_VERSION,
    about = "Personal-development CLI built on Codex. Mode-driven, governance-first.",
    arg_required_else_help = false,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Enter Explore mode (default).
    Explore {
        /// Optional one-shot prompt. Omit to enter an interactive session.
        prompt: Vec<String>,
    },
    /// Enter Plan mode (translate insight into commitments).
    Plan,
    /// Enter Reflect mode (therapy-informed reflection).
    Reflect,
    /// Enter Program mode (twelve-step companion). Eval-gated.
    Program,
    /// Enter Review mode (longitudinal pattern analysis).
    Review,
    /// Initialize .selfwork/ in the current directory.
    Init,
    /// Show current mode, session length, and active risk flags.
    Status,
    /// Open today's journal entry.
    Journal {
        #[command(subcommand)]
        target: JournalTarget,
    },
    /// Display a shared artifact.
    Show {
        #[command(subcommand)]
        target: ShowTarget,
    },
    /// Print the composed system prompt for a mode (debug aid).
    ///
    /// Shows base invariants + mode prompt as the runtime would feed them to
    /// the model. Useful for verifying the governance layer end-to-end.
    Prompt {
        /// Mode whose composed prompt to print.
        mode: String,
    },
    /// Run mode evals.
    Eval {
        /// Mode to evaluate, or `all`.
        target: Option<String>,
        /// Run a single case by id.
        #[arg(long)]
        case: Option<String>,
        /// Print a full report after running.
        #[arg(long)]
        report: bool,
        /// Walk through interactive case authoring.
        #[arg(long)]
        new: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum JournalTarget {
    /// Open today's journal entry.
    Today,
}

#[derive(Debug, Subcommand)]
enum ShowTarget {
    /// Display shared/commitments.md.
    Commitments,
    /// Display recent pattern detections.
    Patterns,
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(async_main())
}

async fn async_main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli
        .command
        .unwrap_or(Command::Explore { prompt: Vec::new() });
    match command {
        Command::Explore { prompt } => run_mode(selfwork_core::Mode::Explore, prompt).await,
        Command::Plan => run_mode(selfwork_core::Mode::Plan, Vec::new()).await,
        Command::Reflect => not_yet_implemented("reflect"),
        Command::Program => not_yet_implemented("program"),
        Command::Review => not_yet_implemented("review"),
        Command::Init => init_workspace(),
        Command::Status => print_status(),
        Command::Journal { target } => match target {
            JournalTarget::Today => journal_today(),
        },
        Command::Show { target } => match target {
            ShowTarget::Commitments => not_yet_implemented("show commitments"),
            ShowTarget::Patterns => not_yet_implemented("show patterns"),
        },
        Command::Eval {
            target,
            case,
            report,
            new,
        } => run_eval_command(target, case, report, new).await,
        Command::Prompt { mode } => print_prompt(&mode),
    }
}

async fn run_eval_command(
    target: Option<String>,
    case: Option<String>,
    report: bool,
    new: Option<String>,
) -> Result<()> {
    if let Some(case_id) = new {
        return not_yet_implemented(&format!("eval --new {case_id}"));
    }
    let root = discover_or_hint()?;
    let target = parse_eval_target(target.as_deref())?;
    let options = selfwork_core::EvalRunOptions {
        target,
        case_id: case,
    };
    let run_report = selfwork_core::run_evals(&root, options)
        .await
        .context("run selfwork evals")?;
    print_eval_report(&run_report, report);
    if !run_report.gate_passed() {
        anyhow::bail!("selfwork eval gate failed");
    }
    Ok(())
}

fn parse_eval_target(target: Option<&str>) -> Result<selfwork_core::EvalTarget> {
    let Some(target) = target else {
        return Ok(selfwork_core::EvalTarget::All);
    };
    if target.eq_ignore_ascii_case("all") {
        return Ok(selfwork_core::EvalTarget::All);
    }
    let Some(mode) = selfwork_core::Mode::from_slug(target) else {
        anyhow::bail!(
            "unknown eval target `{target}` (expected one of: all | explore | plan | reflect | program | review)"
        );
    };
    if !mode.is_implemented() {
        anyhow::bail!(
            "mode `{}` is not implemented yet; available eval targets now: all | explore | plan",
            mode.slug()
        );
    }
    Ok(selfwork_core::EvalTarget::Mode(mode))
}

fn print_eval_report(report: &selfwork_core::EvalRunReport, detailed: bool) {
    println!("Running {} evals...", report.results.len());
    for result in &report.results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        let reason = result
            .failure_reasons
            .first()
            .map(|reason| format!(" - {reason}"))
            .unwrap_or_default();
        println!(
            "{:<11} {:<14} {:<24} {}{}",
            result.mode.slug(),
            result.id,
            result.category,
            status,
            reason
        );
        if detailed && !result.passed {
            for reason in &result.failure_reasons {
                println!("  failure: {reason}");
            }
            println!("  assistant:");
            for line in result.assistant_text.lines() {
                println!("    {line}");
            }
        }
    }
    for summary in report.summaries() {
        let gate = if summary.gate_passed { "PASS" } else { "FAIL" };
        println!(
            "Summary {}: {}/{} passed ({:.0}%, required {:.0}%) gate {}",
            summary.mode.slug(),
            summary.passed,
            summary.total,
            summary.pass_rate * 100.0,
            summary.required_pass_rate * 100.0,
            gate
        );
    }
}

async fn run_mode(mode: selfwork_core::Mode, prompt: Vec<String>) -> Result<()> {
    let root = discover_or_hint()?;
    let builder = selfwork_core::CodexRuntimeBuilder::new(mode)
        .with_cwd(root.workspace.clone())
        .with_selfwork_root(root.clone());
    if !prompt.is_empty() {
        let output = builder
            .run_one_shot(prompt.join(" "))
            .await
            .with_context(|| format!("run {} one-shot", mode.display_name()))?;
        print!("{}", output.assistant_text);
        if !output.assistant_text.ends_with('\n') {
            println!();
        }
        return Ok(());
    }

    interactive_mode_loop(root, builder).await
}

async fn interactive_mode_loop(
    root: selfwork_core::SelfworkRoot,
    builder: selfwork_core::CodexRuntimeBuilder,
) -> Result<()> {
    let mut session = builder
        .start_session()
        .await
        .context("start selfwork session")?;
    mark_active_session(&root, &session)?;
    let mut line = String::new();
    loop {
        eprint!("selfwork[{}]> ", session.mode().slug());
        std::io::stderr().flush()?;
        line.clear();
        let bytes = std::io::stdin().read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        let message = line.trim();
        if message.is_empty() {
            continue;
        }
        if matches!(message, ":end" | ":quit" | ":exit") {
            break;
        }
        if message.starts_with(":switch") {
            session = switch_mode(&root, session, message).await?;
            continue;
        }
        let output = session
            .send_user_message(message.to_string())
            .await
            .with_context(|| format!("send {} turn", session.mode().display_name()))?;
        mark_active_session(&root, &session)?;
        println!("{}", output.assistant_text.trim_end());
    }
    let _ = selfwork_core::write_explore_session_summary(&root, &session)?;
    session
        .shutdown()
        .await
        .context("shutdown selfwork session")?;
    selfwork_core::save_session_state(&root, &selfwork_core::SessionState::default())?;
    Ok(())
}

async fn switch_mode(
    root: &selfwork_core::SelfworkRoot,
    session: selfwork_core::SelfworkSession,
    command: &str,
) -> Result<selfwork_core::SelfworkSession> {
    let target = parse_switch_target(command)?;
    if !target.is_implemented() {
        anyhow::bail!(
            "mode `{}` is not implemented yet; available now: explore | plan",
            target.slug()
        );
    }
    if target == session.mode() {
        eprintln!("selfwork: already in {} mode.", target.display_name());
        return Ok(session);
    }

    let handoff = selfwork_core::compile_handoff(root, session.mode(), target, session.turns())
        .context("compile mode handoff")?;
    let migration_path =
        selfwork_core::write_migration(root, &handoff).context("write handoff migration")?;
    let _ = selfwork_core::write_explore_session_summary(root, &session)?;
    session
        .shutdown()
        .await
        .context("shutdown outgoing mode session")?;

    let developer_instructions = selfwork_core::render_handoff_developer_instructions(&handoff)
        .context("render incoming handoff instructions")?;
    let incoming = selfwork_core::CodexRuntimeBuilder::new(target)
        .with_cwd(root.workspace.clone())
        .with_selfwork_root(root.clone())
        .with_developer_instructions(developer_instructions)
        .start_session()
        .await
        .with_context(|| format!("start {} session", target.display_name()))?;
    mark_active_session(root, &incoming)?;
    eprintln!(
        "selfwork: switched to {} mode with handoff {}",
        target.display_name(),
        migration_path.display()
    );
    Ok(incoming)
}

fn parse_switch_target(command: &str) -> Result<selfwork_core::Mode> {
    let mut parts = command.split_whitespace();
    let directive = parts.next().unwrap_or_default();
    if directive != ":switch" {
        anyhow::bail!("expected :switch <mode>");
    }
    let Some(mode) = parts.next() else {
        anyhow::bail!("usage: :switch <explore|plan|reflect|program|review>");
    };
    if parts.next().is_some() {
        anyhow::bail!("usage: :switch <explore|plan|reflect|program|review>");
    }
    selfwork_core::Mode::from_slug(mode).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown mode `{mode}` (expected one of: explore | plan | reflect | program | review)"
        )
    })
}

fn mark_active_session(
    root: &selfwork_core::SelfworkRoot,
    session: &selfwork_core::SelfworkSession,
) -> Result<()> {
    selfwork_core::save_active_mode(
        root,
        &selfwork_core::ActiveMode {
            mode: session.mode().slug().to_string(),
            entered_at: Some(Utc::now()),
        },
    )?;
    selfwork_core::save_session_state(
        root,
        &selfwork_core::SessionState {
            session_id: Some(session.thread_id().to_string()),
            started_at: Some(Utc::now()),
            turns: session.turns().len() as u64,
        },
    )?;
    Ok(())
}

fn print_prompt(mode_name: &str) -> Result<()> {
    use selfwork_core::Mode;
    use selfwork_core::ModeBundle;

    let mode = match mode_name.to_ascii_lowercase().as_str() {
        "explore" => Mode::Explore,
        "plan" => Mode::Plan,
        "reflect" => Mode::Reflect,
        "program" => Mode::Program,
        "review" => Mode::Review,
        other => {
            anyhow::bail!(
                "unknown mode `{other}` (expected one of: explore | plan | reflect | program | review)"
            );
        }
    };
    match ModeBundle::for_mode(mode) {
        Some(bundle) => {
            print!("{}", bundle.render_system_prompt());
            Ok(())
        }
        None => {
            anyhow::bail!(
                "mode `{}` is not yet implemented in this build (available now: Explore and Plan)",
                mode.display_name()
            );
        }
    }
}

fn not_yet_implemented(command: &str) -> Result<()> {
    eprintln!(
        "selfwork: `{command}` is scaffolded but not yet implemented. \
         Step 0 of the migration plan ships the CLI surface; later steps fill in behavior."
    );
    Ok(())
}

fn print_status() -> Result<()> {
    let root = discover_or_hint()?;
    let active = selfwork_core::load_active_mode(&root)?;
    let session = selfwork_core::load_session_state(&root)?;
    let risk = selfwork_core::load_risk_flags(&root)?;

    let active_label = selfwork_core::Mode::from_slug(&active.mode)
        .map(|m| m.display_name().to_string())
        .unwrap_or_else(|| format!("{} (unknown)", active.mode));
    let entered = active
        .entered_at
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_else(|| "not yet started".to_string());
    let session_label = match session.started_at {
        Some(started) => format!(
            "{} ({} turns, started {})",
            session.session_id.as_deref().unwrap_or("active"),
            session.turns,
            started.to_rfc3339()
        ),
        None => "none".to_string(),
    };
    let risk_label = if risk.flags.is_empty() {
        "none".to_string()
    } else {
        risk.flags.join(", ")
    };

    println!("workspace:    {}", root.root.display());
    println!("active mode:  {active_label} (entered: {entered})");
    println!("session:      {session_label}");
    println!("risk flags:   {risk_label}");
    Ok(())
}

fn discover_or_hint() -> Result<selfwork_core::SelfworkRoot> {
    let cwd = std::env::current_dir()?;
    selfwork_core::discover_root(&cwd).ok_or_else(|| {
        anyhow::anyhow!(
            "no .selfwork/ workspace found upward from current directory; run `selfwork init` first"
        )
    })
}

fn journal_today() -> Result<()> {
    let root = discover_or_hint()?;
    let today = chrono::Local::now().date_naive();
    let entry = selfwork_core::ensure_journal_entry(&root, today)?;
    if entry.created {
        eprintln!("selfwork: created {}", entry.path.display());
    }
    println!("{}", entry.path.display());
    if let Ok(editor) = std::env::var("EDITOR")
        && !editor.is_empty()
    {
        match std::process::Command::new(&editor)
            .arg(&entry.path)
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(status) => {
                anyhow::bail!("$EDITOR ({editor}) exited with status {status}")
            }
            Err(err) => {
                eprintln!("selfwork: failed to launch $EDITOR ({editor}): {err}");
            }
        }
    }
    Ok(())
}

fn init_workspace() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let report = selfwork_core::bootstrap_workspace(&cwd)?;
    if report.root_already_existed {
        if report.nothing_was_created() {
            println!(
                "selfwork: workspace at {} already up to date.",
                report.root.display()
            );
        } else {
            println!(
                "selfwork: workspace at {} topped up.",
                report.root.display()
            );
        }
    } else {
        println!(
            "selfwork: initialized workspace at {}.",
            report.root.display()
        );
    }
    if !report.created_directories.is_empty() {
        println!(
            "  directories created: {}",
            report.created_directories.len()
        );
    }
    if !report.created_files.is_empty() {
        println!("  files seeded:        {}", report.created_files.len());
    }
    Ok(())
}
