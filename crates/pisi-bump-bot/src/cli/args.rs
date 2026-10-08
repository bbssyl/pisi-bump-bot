use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::prepare_runner::DEFAULT_LIMIT;

#[derive(Debug, Parser)]
#[command(name = "pisi-bump-bot")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Report(ReportArgs),
    Prepare(PrepareArgs),
    MergeResults(MergeResultsArgs),
    Render(RenderArgs),
}

#[derive(Debug, Parser)]
pub struct ReportArgs {
    #[arg(long)]
    pub recipes_dir: PathBuf,
    #[arg(long = "json")]
    pub json_path: PathBuf,
    #[arg(long = "markdown")]
    pub markdown_path: PathBuf,
    #[arg(long = "previous")]
    pub previous_path: Option<PathBuf>,
    #[arg(long = "new-updates")]
    pub new_updates_path: Option<PathBuf>,
    #[arg(long)]
    pub source_commit: Option<String>,
    #[arg(long)]
    pub with_hash: bool,
}

#[derive(Debug, Parser)]
pub struct PrepareArgs {
    #[arg(long)]
    pub report: PathBuf,
    #[arg(long)]
    pub recipes_dir: PathBuf,
    #[arg(long)]
    pub state: PathBuf,
    #[arg(long)]
    pub output_dir: PathBuf,
    #[arg(long)]
    pub build_list: PathBuf,
    #[arg(long)]
    pub run_date: Option<String>,
    #[arg(long, default_value_t = DEFAULT_LIMIT)]
    pub limit: usize,
}

#[derive(Debug, Parser)]
pub struct MergeResultsArgs {
    #[arg(long)]
    pub state: PathBuf,
    #[arg(long)]
    pub results_dir: PathBuf,
    #[arg(long)]
    pub comment: PathBuf,
    #[arg(long)]
    pub run_date: Option<String>,
    #[arg(long)]
    pub repo_web_url: Option<String>,
}

#[derive(Debug, Parser)]
pub struct RenderArgs {
    #[arg(long)]
    pub report: PathBuf,
    #[arg(long)]
    pub state: PathBuf,
    #[arg(long)]
    pub markdown: PathBuf,
    #[arg(long)]
    pub repo_web_url: Option<String>,
    #[arg(long)]
    pub relative_files: bool,
}
