//! Selfwork CLI entry point.
//!
//! v1 surface (spec §7). Step 0: stub commands so `--help` documents the
//! shape we are building toward; subsequent migration steps fill them in.

use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

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
    Explore,
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
        target: String,
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
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Explore);
    match command {
        Command::Explore => not_yet_implemented("explore"),
        Command::Plan => not_yet_implemented("plan"),
        Command::Reflect => not_yet_implemented("reflect"),
        Command::Program => not_yet_implemented("program"),
        Command::Review => not_yet_implemented("review"),
        Command::Init => init_workspace(),
        Command::Status => not_yet_implemented("status"),
        Command::Journal { target } => match target {
            JournalTarget::Today => not_yet_implemented("journal today"),
        },
        Command::Show { target } => match target {
            ShowTarget::Commitments => not_yet_implemented("show commitments"),
            ShowTarget::Patterns => not_yet_implemented("show patterns"),
        },
        Command::Eval { target, .. } => not_yet_implemented(&format!("eval {target}")),
        Command::Prompt { mode } => print_prompt(&mode),
    }
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
                "mode `{}` is not yet implemented in this build (Step 1 ships Explore only)",
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
        println!("  directories created: {}", report.created_directories.len());
    }
    if !report.created_files.is_empty() {
        println!("  files seeded:        {}", report.created_files.len());
    }
    Ok(())
}
