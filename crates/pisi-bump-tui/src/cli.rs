use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "pisi-bump-tui",
    about = "Pisi contrib paketleri için etkileşimli sürüm takip aracı"
)]
pub struct Cli {
    #[arg(long)]
    pub recipes_dir: PathBuf,

    #[arg(long)]
    pub token: Option<String>,
}
