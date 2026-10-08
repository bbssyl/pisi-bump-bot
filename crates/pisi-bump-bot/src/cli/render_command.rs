use crate::build_columns::BuildLinks;
use crate::build_state::load_state;
use crate::cli::args::RenderArgs;
use crate::report_loader::load_report;
use crate::report_writer::render_markdown;

fn write_text(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

pub fn run(args: RenderArgs) -> i32 {
    let report = match load_report(&args.report) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("render başarısız: {error}");
            return 1;
        }
    };
    let state = match load_state(&args.state) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("render başarısız: {error}");
            return 1;
        }
    };
    let links = BuildLinks::new(state, args.repo_web_url.clone(), args.relative_files);
    let markdown = render_markdown(&report, Some(&links));
    if write_text(&args.markdown, &markdown).is_err() {
        eprintln!("render başarısız: markdown dosyası yazılamadı");
        return 1;
    }
    0
}
