//! modsh — Modern shell with AI context
//!
//! Entry point for the modsh binary.

use anyhow::Result;
use clap::Parser;
use std::io::{self, Read};
use std::path::PathBuf;

/// modsh CLI arguments
#[derive(Parser, Debug)]
#[command(name = "modsh")]
#[command(about = "Modern shell with AI context")]
#[command(version)]
struct Args {
    /// Command to execute (if not provided, starts interactive shell)
    #[arg(short, long)]
    command: Option<String>,
    /// Execute script file
    #[arg(short, long)]
    file: Option<PathBuf>,
    /// Run in POSIX strict mode
    #[arg(long)]
    posix: bool,
    /// Disable AI features
    #[arg(long)]
    no_ai: bool,
    /// Disable interactive features
    #[arg(long)]
    no_interactive: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load config
    let config = load_config()?;

    // Execute command mode
    if let Some(cmd) = args.command {
        return run_command(&cmd, &config).await;
    }

    // Execute script file
    if let Some(file) = args.file {
        return run_script(&file, &config).await;
    }

    // Interactive mode
    if !args.no_interactive && atty::is(atty::Stream::Stdin) {
        run_interactive(&config, args.no_ai).await?;
    } else {
        // Non-interactive: read from stdin
        run_stdin(&config).await?;
    }

    Ok(())
}

async fn run_command(cmd: &str, _config: &Config) -> Result<()> {
    use modsh_core::executor::Executor;
    use modsh_core::parser::parse;

    let ast = parse(cmd)?;
    let mut executor = Executor::new();
    let status = executor.execute(&ast)?;

    if !status.success() {
        std::process::exit(status.code as i32);
    }

    Ok(())
}

async fn run_script(file: &PathBuf, _config: &Config) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        run_command(trimmed, _config).await?;
    }

    Ok(())
}

async fn run_interactive(config: &Config, _no_ai: bool) -> Result<()> {
    use modsh_interactive::editor::LineEditor;
    use modsh_interactive::prompt::{PromptEngine, PromptConfig};
    use modsh_interactive::history::HistoryEngine;

    let mut editor = LineEditor::new();
    let mut prompt = PromptEngine::new(PromptConfig::default());
    let mut history = HistoryEngine::new();

    // Load history
    let history_file = dirs::data_dir()
        .map(|d| d.join("modsh/history"))
        .unwrap_or_else(|| PathBuf::from(".modsh_history"));
    history.set_history_file(history_file.clone());
    let _ = history.load();

    println!("modsh {} — Modern shell with AI context", env!("CARGO_PKG_VERSION"));
    println!("Type 'exit' to quit\n");

    loop {
        let prompt_str = prompt.render();
        
        match editor.read_line(&prompt_str) {
            Ok(line) => {
                let trimmed = line.trim();
                
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "exit" {
                    break;
                }

                // Execute
                let start = std::time::Instant::now();
                
                match run_command(trimmed, config).await {
                    Ok(()) => {
                        let duration = start.elapsed().as_millis() as u64;
                        history.add_command(line.clone(), 0, duration);
                        prompt.set_exit_code(0);
                    }
                    Err(e) => {
                        let duration = start.elapsed().as_millis() as u64;
                        eprintln!("Error: {}", e);
                        history.add_command(line.clone(), 1, duration);
                        prompt.set_exit_code(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }

    // Save history
    let _ = history.save();

    Ok(())
}

async fn run_stdin(_config: &Config) -> Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    for line in buffer.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Execute each line
        // TODO: Batch execution for efficiency
    }

    Ok(())
}

/// Configuration for modsh
#[derive(Debug, Clone)]
struct Config {
    posix_strict: bool,
    ai_enabled: bool,
    interactive_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            posix_strict: false,
            ai_enabled: true,
            interactive_enabled: true,
        }
    }
}

fn load_config() -> Result<Config> {
    // TODO: Load from ~/.config/modsh/config.toml
    Ok(Config::default())
}

// Simple atty replacement
mod atty {
    pub enum Stream {
        Stdin,
        Stdout,
        Stderr,
    }

    pub fn is(stream: Stream) -> bool {
        match stream {
            Stream::Stdin => unsafe { libc::isatty(0) != 0 },
            Stream::Stdout => unsafe { libc::isatty(1) != 0 },
            Stream::Stderr => unsafe { libc::isatty(2) != 0 },
        }
    }
}

mod dirs {
    use std::path::PathBuf;

    pub fn data_dir() -> Option<PathBuf> {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local/share"))
            })
    }
}
