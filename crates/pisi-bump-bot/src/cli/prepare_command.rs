use crate::build_state::{load_state, save_state};
use crate::cli::args::PrepareArgs;
use crate::prepare_runner::{Downloader, PrepareReport, PrepareSettings, run_prepare};
use crate::report_loader::load_report;

fn today() -> String {
    let date = time::OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

pub fn run(args: PrepareArgs, download: Box<Downloader<'static>>) -> i32 {
    let report = match load_report(&args.report) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("prepare başarısız: {error}");
            return 1;
        }
    };
    let state = match load_state(&args.state) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("prepare başarısız: {error}");
            return 1;
        }
    };
    let prepare_report = PrepareReport::from_report(&report);
    let run_date = args.run_date.clone().unwrap_or_else(today);
    let settings =
        PrepareSettings::new(args.recipes_dir.clone(), args.output_dir.clone(), run_date)
            .with_limit(args.limit);
    let outcome = run_prepare(&prepare_report, state, &settings, download.as_ref());
    if let Err(error) = save_state(&args.state, &outcome.state) {
        eprintln!("prepare başarısız: {error}");
        return 1;
    }
    let build_list_json = python_json_string_list(&outcome.build_list);
    if write_text(&args.build_list, &build_list_json).is_err() {
        eprintln!("prepare başarısız: derlenecek liste yazılamadı");
        return 1;
    }
    eprintln!("derlenecek paket sayısı: {}", outcome.build_list.len());
    0
}

fn python_json_string_list(items: &[String]) -> String {
    let encoded: Vec<String> = items.iter().map(|item| ascii_json_string(item)).collect();
    format!("[{}]", encoded.join(", "))
}

fn ascii_json_string(text: &str) -> String {
    let escaped = serde_json::to_string(text).expect("string must serialize");
    let mut output = String::with_capacity(escaped.len());
    for character in escaped.chars() {
        if character.is_ascii() {
            output.push(character);
            continue;
        }
        let mut units = [0u16; 2];
        for unit in character.encode_utf16(&mut units) {
            output.push_str(&format!("\\u{unit:04x}"));
        }
    }
    output
}

fn write_text(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_match_python_json_dumps_when_list_is_empty() {
        assert_eq!(python_json_string_list(&[]), "[]");
    }

    #[test]
    fn should_match_python_json_dumps_separators_when_list_has_paths() {
        let items = vec![
            "network/browser/brave".to_string(),
            "pot-desktop".to_string(),
        ];

        let encoded = python_json_string_list(&items);

        assert_eq!(encoded, r#"["network/browser/brave", "pot-desktop"]"#);
    }

    #[test]
    fn should_escape_non_ascii_like_python_ensure_ascii_when_path_is_unusual() {
        let items = vec!["oyun/şah\"\u{1F600}".to_string()];

        let encoded = python_json_string_list(&items);

        let python_reference = format!("[\"oyun/{0}u015fah\\\"{0}ud83d{0}ude00\"]", '\\');
        assert_eq!(encoded, python_reference);
    }
}
