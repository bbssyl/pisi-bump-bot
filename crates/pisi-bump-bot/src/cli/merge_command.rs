use crate::build_results::{load_results, merge_results, render_results_comment};
use crate::build_state::{load_state, save_state};
use crate::cli::args::MergeResultsArgs;

fn today() -> String {
    let date = time::OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

fn write_text(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

pub fn run(args: MergeResultsArgs) -> i32 {
    let state = match load_state(&args.state) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("merge-results başarısız: {error}");
            return 1;
        }
    };
    let results = load_results(&args.results_dir);
    let run_date = args.run_date.clone().unwrap_or_else(today);
    let (merged, applied) = merge_results(&state, &results, &run_date);
    if let Err(error) = save_state(&args.state, &merged) {
        eprintln!("merge-results başarısız: {error}");
        return 1;
    }
    let comment = render_results_comment(&applied, args.repo_web_url.as_deref());
    if write_text(&args.comment, &comment).is_err() {
        eprintln!("merge-results başarısız: yorum dosyası yazılamadı");
        return 1;
    }
    0
}
