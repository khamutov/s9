//! Iteration loop and Claude subprocess orchestration.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, bail};
use chrono::Utc;

use crate::cli::Cli;
use crate::tasks::{check_completion, find_next_task, parse_tasks};

const COMPLETION_SENTINEL: &str = "<starpom>COMPLETE</starpom>";

/// Resolve the git repository root.
fn repo_root() -> anyhow::Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!("not inside a git repository");
    }
    Ok(PathBuf::from(
        String::from_utf8(output.stdout)?.trim().to_string(),
    ))
}

/// Run pre-flight checks before starting the loop.
fn preflight(root: &Path) -> anyhow::Result<()> {
    // Check claude is available in PATH
    let found = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join("claude");
                candidate.is_file() || candidate.with_extension("exe").is_file()
            })
        })
        .unwrap_or(false);
    if !found {
        bail!("'claude' not found in PATH.");
    }

    // Warn on uncommitted changes
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .context("failed to run git status")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        eprintln!("WARNING: Uncommitted changes detected. Proceeding anyway.\n");
    }

    // Check TASKS.md exists
    let tasks_path = root.join("TASKS.md");
    if !tasks_path.exists() {
        bail!("{} not found.", tasks_path.display());
    }

    Ok(())
}

/// Load prompt.md and append phase focus suffix if needed.
fn build_prompt(target_phase: Option<u32>) -> anyhow::Result<String> {
    let prompt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("prompt.md");
    let mut prompt = std::fs::read_to_string(&prompt_path).context("failed to read prompt.md")?;

    if let Some(phase) = target_phase {
        prompt.push_str(&format!(
            "\n\n**PHASE FOCUS**: Only work on Phase {phase} tasks.\n"
        ));
    }

    Ok(prompt)
}

/// Run a single Claude invocation, streaming output line-by-line.
///
/// Uses `duct` to merge stderr into stdout, avoiding pipe-buffer deadlocks.
/// Returns the full captured output. Checks `interrupted` between lines
/// and kills the child process on interrupt.
fn run_iteration(
    prompt: &str,
    root: &Path,
    interrupted: &Arc<AtomicBool>,
) -> anyhow::Result<String> {
    let reader = duct::cmd(
        "claude",
        &[
            "--dangerously-skip-permissions",
            "--chrome",
            "--print",
            prompt,
        ],
    )
    .dir(root)
    .stderr_to_stdout()
    .unchecked()
    .reader()
    .context("failed to spawn claude")?;

    let buf = BufReader::new(&reader);
    let mut lines = Vec::new();

    for line in buf.lines() {
        if interrupted.load(Ordering::Relaxed) {
            reader.kill()?;
            bail!("interrupted");
        }
        let line = line.context("failed to read line")?;
        println!("{line}");
        lines.push(line);
    }

    Ok(lines.join("\n"))
}

/// Sleep for `duration_ms` in small increments, checking for interrupts.
/// Returns `true` if interrupted.
fn interruptible_sleep(duration_ms: u64, interrupted: &Arc<AtomicBool>) -> bool {
    let chunks = duration_ms / 100;
    for _ in 0..chunks {
        if interrupted.load(Ordering::Relaxed) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

/// Main loop: pre-flight checks, iterate, summarize.
pub fn run_loop(args: &Cli, interrupted: &Arc<AtomicBool>) -> anyhow::Result<()> {
    let root = repo_root()?;
    preflight(&root)?;

    let tasks_path = root.join("TASKS.md");
    let progress_path = root.join("progress.txt");

    // Handle --clean or missing progress.txt
    if args.clean || !progress_path.exists() {
        let ts = Utc::now().to_rfc3339();
        std::fs::write(
            &progress_path,
            format!("# Starpom Progress Log\n\nStarted: {ts}\n\n"),
        )?;
        println!("Initialized progress.txt");
    }

    // Parse tasks and check completion
    let tasks = parse_tasks(&tasks_path)?;
    if check_completion(&tasks, args.phase) {
        let label = match args.phase {
            Some(p) => format!("Phase {p}"),
            None => "All".to_string(),
        };
        println!("{label} tasks already completed.");
        return Ok(());
    }

    // Show next task
    match find_next_task(&tasks, args.phase) {
        Some(t) => println!("Next task: {} — {}", t.id, t.description),
        None => {
            println!("No actionable tasks found (all remaining are blocked).");
            return Ok(());
        }
    }

    // Build prompt
    let prompt = build_prompt(args.phase)?;

    // Handle --dry-run
    if args.dry_run {
        println!("\n--- DRY RUN: Prompt that would be sent ---\n");
        println!("{prompt}");
        return Ok(());
    }

    // Main loop
    let mut completed: u32 = 0;
    for i in 1..=args.max_iterations {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }

        let ts = Utc::now().format("%H:%M:%S UTC");
        println!("\n{}", "=".repeat(60));
        println!("  Iteration {i}/{}  |  {ts}", args.max_iterations);
        println!("{}\n", "=".repeat(60));

        let output = run_iteration(&prompt, &root, interrupted)?;
        completed += 1;

        // Check for completion sentinel
        if output.contains(COMPLETION_SENTINEL) {
            println!("\nAgent signaled COMPLETE.");
            break;
        }

        // Re-parse tasks and check
        let tasks = parse_tasks(&tasks_path)?;
        if check_completion(&tasks, args.phase) {
            let label = match args.phase {
                Some(p) => format!("Phase {p}"),
                None => "All".to_string(),
            };
            println!("\n{label} tasks completed.");
            break;
        }

        match find_next_task(&tasks, args.phase) {
            Some(t) => println!("\nNext task: {} — {}", t.id, t.description),
            None => {
                println!("\nNo more actionable tasks.");
                break;
            }
        }

        // Brief pause between iterations (interruptible)
        if i < args.max_iterations && interruptible_sleep(3000, interrupted) {
            break;
        }
    }

    // Summary
    println!("\n{}", "=".repeat(60));
    println!("  Starpom finished: {completed} iteration(s) completed");
    println!("{}", "=".repeat(60));

    Ok(())
}
