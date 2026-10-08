use std::env;

use pisi_bump_bot::cli::{self, Runtime};

const TOKEN_VARIABLE: &str = "GITHUB_TOKEN";

fn main() {
    let runtime = Runtime::production(env::var(TOKEN_VARIABLE).ok());
    let exit_code = cli::run(env::args(), runtime);
    std::process::exit(exit_code);
}
