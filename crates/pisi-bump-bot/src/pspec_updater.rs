use std::sync::LazyLock;

use regex::Regex;

use crate::difflib;
use crate::error::PspecUpdateError;

const AUTHOR_NAME: &str = "pisi-bump-bot";
const AUTHOR_EMAIL: &str = "pisi-bump-bot@users.noreply.github.com";
const CHILD_TAGS: [&str; 5] = ["Date", "Version", "Comment", "Name", "Email"];
const DEFAULT_CHILD_INDENT: &str = "    ";
const LEADING_NOISE: &[char] = &['\u{feff}', ' ', '\t', '\r', '\n'];

static ARCHIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(<Archive\b[^>]*>)(\s*)([^<]*?)(\s*)(</Archive>)").unwrap());
static SHA1_ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(sha1sum\s*=\s*)(?:(")[^"']*"|(')[^"']*')"#).unwrap());
static HISTORY_UPDATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<History\b[^>]*>.*?(<Update\b[^>]*>)").unwrap());
static CLOSING_UPDATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^([ \t]*)</Update>").unwrap());

pub struct UpdateRequest<'a> {
    pub recipe_path: &'a str,
    pub pspec_text: &'a str,
    pub actions_text: Option<&'a str>,
    pub new_version: &'a str,
    pub new_archive_url: &'a str,
    pub new_sha1: &'a str,
    pub run_date: &'a str,
    pub old_archive_url: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUpdate {
    pub recipe_path: String,
    pub new_pspec_text: String,
    pub unified_diff: String,
    pub warnings: Vec<&'static str>,
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('>', "&gt;")
        .replace('<', "&lt;")
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn parse_document(text: &str) -> Result<roxmltree::Document<'_>, PspecUpdateError> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    Ok(roxmltree::Document::parse_with_options(
        text.trim_start_matches(LEADING_NOISE),
        options,
    )?)
}

fn children<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    tag: &'static str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    node.children().filter(move |child| child.has_tag_name(tag))
}

fn find_text(node: roxmltree::Node, tag: &'static str) -> String {
    children(node, tag)
        .next()
        .and_then(|child| child.text())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn release_number(update: roxmltree::Node) -> u32 {
    update
        .attribute("release")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn history_updates<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    children(root, "History")
        .flat_map(|history| children(history, "Update"))
        .collect()
}

fn source_archives<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    children(root, "Source")
        .flat_map(|source| children(source, "Archive"))
        .collect()
}

fn newest_update<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
) -> Option<roxmltree::Node<'a, 'input>> {
    let updates = history_updates(root);
    let best = updates.iter().map(|update| release_number(*update)).max()?;
    updates
        .into_iter()
        .find(|update| release_number(*update) == best)
}

fn line_indent(text: &str, position: usize) -> Result<&str, PspecUpdateError> {
    let line_start = text[..position].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &text[line_start..position];
    if !prefix.trim().is_empty() {
        return Err(PspecUpdateError::UnexpectedIndent);
    }
    Ok(prefix)
}

fn with_new_sha1(opening: &str, sha1: &str) -> Result<String, PspecUpdateError> {
    let found = SHA1_ATTRIBUTE
        .captures(opening)
        .ok_or(PspecUpdateError::MissingSha1)?;
    let quote = found.get(2).or_else(|| found.get(3)).unwrap().as_str();
    let whole = found.get(0).unwrap();
    Ok(format!(
        "{}{}{quote}{sha1}{quote}{}",
        &opening[..whole.start()],
        &found[1],
        &opening[whole.end()..]
    ))
}

fn replace_archive(text: &str, request: &UpdateRequest) -> Result<String, PspecUpdateError> {
    for found in ARCHIVE.captures_iter(text) {
        if request
            .old_archive_url
            .is_some_and(|old| xml_unescape(&found[3]) != old)
        {
            continue;
        }
        let opening = with_new_sha1(&found[1], request.new_sha1)?;
        let url = xml_escape(request.new_archive_url);
        let whole = found.get(0).unwrap();
        let replacement = format!("{opening}{}{url}{}{}", &found[2], &found[4], &found[5]);
        return Ok(format!(
            "{}{replacement}{}",
            &text[..whole.start()],
            &text[whole.end()..]
        ));
    }
    Err(PspecUpdateError::NoMatchingArchive)
}

fn child_indent(block: &str, tag: &str, fallback: &str) -> String {
    let pattern = Regex::new(&format!(r"(?m)^([ \t]*)<{tag}>")).unwrap();
    pattern
        .captures(block)
        .map_or_else(|| fallback.to_string(), |found| found[1].to_string())
}

fn build_update_block(
    existing: &str,
    opening_indent: &str,
    release: u32,
    request: &UpdateRequest,
) -> String {
    let newline = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let comment = format!("Version bump to {}", request.new_version);
    let values = [
        request.run_date,
        request.new_version,
        comment.as_str(),
        AUTHOR_NAME,
        AUTHOR_EMAIL,
    ];
    let fallback = format!("{opening_indent}{DEFAULT_CHILD_INDENT}");
    let mut lines = vec![format!("{opening_indent}<Update release=\"{release}\">")];
    for (tag, value) in CHILD_TAGS.iter().zip(values) {
        let indent = child_indent(existing, tag, &fallback);
        lines.push(format!("{indent}<{tag}>{}</{tag}>", xml_escape(value)));
    }
    let closing = CLOSING_UPDATE
        .captures(existing)
        .map_or(opening_indent.to_string(), |found| found[1].to_string());
    lines.push(format!("{closing}</Update>"));
    lines.join(newline) + newline
}

fn insert_history_entry(
    text: &str,
    release: u32,
    request: &UpdateRequest,
) -> Result<String, PspecUpdateError> {
    let found = HISTORY_UPDATE
        .captures(text)
        .ok_or(PspecUpdateError::NoHistoryUpdate)?;
    let start = found.get(1).unwrap().start();
    let end = text[start..]
        .find("</Update>")
        .map(|offset| start + offset)
        .ok_or(PspecUpdateError::UnclosedUpdate)?;
    let line_start = text[..start].rfind('\n').map_or(0, |index| index + 1);
    let existing = &text[line_start..end + "</Update>".len()];
    let block = build_update_block(existing, line_indent(text, start)?, release, request);
    Ok(format!(
        "{}{block}{}",
        &text[..line_start],
        &text[line_start..]
    ))
}

fn validate_result(
    new_text: &str,
    release: u32,
    request: &UpdateRequest,
) -> Result<(), PspecUpdateError> {
    let document = parse_document(new_text)?;
    let root = document.root_element();
    let latest = newest_update(root).ok_or(PspecUpdateError::HistoryNotValidated)?;
    if release_number(latest) != release || find_text(latest, "Version") != request.new_version {
        return Err(PspecUpdateError::HistoryNotValidated);
    }
    let matched = source_archives(root).iter().any(|archive| {
        archive.attribute("sha1sum") == Some(request.new_sha1)
            && archive.text().unwrap_or("").trim() == request.new_archive_url
    });
    if matched {
        Ok(())
    } else {
        Err(PspecUpdateError::ArchiveNotValidated)
    }
}

fn version_present(text: &str, version: &str) -> bool {
    let variants = [version.to_string(), version.replace('.', "_")];
    variants.iter().any(|variant| {
        let pattern = format!(r"(?<![\w.]){}(?![\w]|\.\d)", fancy_regex::escape(variant));
        fancy_regex::Regex::new(&pattern)
            .unwrap()
            .is_match(text)
            .unwrap()
    })
}

fn collect_warnings(root: roxmltree::Node, request: &UpdateRequest) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    let old_version = newest_update(root)
        .map(|update| find_text(update, "Version"))
        .unwrap_or_default();
    if let Some(actions) = request.actions_text.filter(|actions| !actions.is_empty())
        && !old_version.is_empty()
        && version_present(actions, &old_version)
    {
        warnings.push("actions.py içinde sürüm elle yazılmış, elle kontrol edilmeli");
    }
    if source_archives(root).len() > 1 {
        warnings.push("birden fazla Archive var, sadece eşleşen güncellendi");
    }
    if request.pspec_text.contains('\t') {
        warnings.push("dosyada sekme karakteri var (Pisi kuralı: boşluk kullanılmalı)");
    }
    warnings
}

pub fn split_lines_keep_lf(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

pub fn make_diff(recipe_path: &str, old_text: &str, new_text: &str) -> String {
    let label = format!("{recipe_path}/pspec.xml");
    let old_lines = split_lines_keep_lf(old_text);
    let new_lines = split_lines_keep_lf(new_text);
    difflib::unified_diff(
        &old_lines,
        &new_lines,
        &format!("a/{label}"),
        &format!("b/{label}"),
        3,
    )
    .into_iter()
    .map(|line| {
        if line.ends_with('\n') {
            line
        } else {
            line + "\n"
        }
    })
    .collect()
}

pub fn prepare_update(request: &UpdateRequest) -> Result<PreparedUpdate, PspecUpdateError> {
    let document = parse_document(request.pspec_text)?;
    let root = document.root_element();
    let release = history_updates(root)
        .iter()
        .map(|update| release_number(*update))
        .max()
        .unwrap_or(0)
        + 1;
    let with_archive = replace_archive(request.pspec_text, request)?;
    let new_text = insert_history_entry(&with_archive, release, request)?;
    validate_result(&new_text, release, request)?;
    let unified_diff = make_diff(request.recipe_path, request.pspec_text, &new_text);
    Ok(PreparedUpdate {
        recipe_path: request.recipe_path.to_string(),
        new_pspec_text: new_text,
        unified_diff,
        warnings: collect_warnings(root, request),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    const FAKE_SHA1: &str = "0123456789abcdef0123456789abcdef01234567";
    const RUN_DATE: &str = "2026-10-07";

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures/pspec")
    }

    fn read_fixture(name: &str) -> String {
        fs::read_to_string(fixtures_dir().join(name)).unwrap()
    }

    struct RequestBuilder {
        recipe_path: String,
        pspec_text: String,
        actions_text: Option<String>,
        new_version: String,
        new_archive_url: String,
        new_sha1: String,
        run_date: String,
        old_archive_url: Option<String>,
    }

    impl RequestBuilder {
        fn new(pspec_text: impl Into<String>) -> Self {
            Self {
                recipe_path: "network/browser/brave".to_string(),
                pspec_text: pspec_text.into(),
                actions_text: None,
                new_version: "1.94.1".to_string(),
                new_archive_url: "https://example.org/new-1.94.1.zip".to_string(),
                new_sha1: FAKE_SHA1.to_string(),
                run_date: RUN_DATE.to_string(),
                old_archive_url: None,
            }
        }

        fn new_version(mut self, value: &str) -> Self {
            self.new_version = value.to_string();
            self
        }

        fn new_archive_url(mut self, value: &str) -> Self {
            self.new_archive_url = value.to_string();
            self
        }

        fn actions_text(mut self, value: &str) -> Self {
            self.actions_text = Some(value.to_string());
            self
        }

        fn old_archive_url(mut self, value: &str) -> Self {
            self.old_archive_url = Some(value.to_string());
            self
        }

        fn request(&self) -> UpdateRequest<'_> {
            UpdateRequest {
                recipe_path: &self.recipe_path,
                pspec_text: &self.pspec_text,
                actions_text: self.actions_text.as_deref(),
                new_version: &self.new_version,
                new_archive_url: &self.new_archive_url,
                new_sha1: &self.new_sha1,
                run_date: &self.run_date,
                old_archive_url: self.old_archive_url.as_deref(),
            }
        }
    }

    fn changed_lines(diff: &str, marker: char) -> Vec<&str> {
        diff.lines()
            .filter(|line| line.starts_with(marker) && line.chars().nth(1) != Some(marker))
            .map(|line| &line[1..])
            .collect()
    }

    #[test]
    fn should_touch_only_archive_and_new_history_block_when_brave() {
        let builder = RequestBuilder::new(read_fixture("brave.xml"));
        let result = prepare_update(&builder.request()).unwrap();
        let removed = changed_lines(&result.unified_diff, '-');
        let added = changed_lines(&result.unified_diff, '+');
        assert_eq!(removed.len(), 2);
        assert_eq!(added.len(), 9);
        assert!(added[1].contains("https://example.org/new-1.94.1.zip"));
    }

    #[test]
    fn should_preserve_newline_wrapped_url_layout_when_brave() {
        let builder = RequestBuilder::new(read_fixture("brave.xml"));
        let result = prepare_update(&builder.request()).unwrap();
        let expected = format!(
            "<Archive sha1sum=\"{FAKE_SHA1}\" type=\"binary\">\n           https://example.org/new-1.94.1.zip\n        </Archive>"
        );
        assert!(result.new_pspec_text.contains(&expected));
    }

    #[test]
    fn should_insert_release_four_with_author_when_brave() {
        let builder = RequestBuilder::new(read_fixture("brave.xml"));
        let result = prepare_update(&builder.request()).unwrap();
        let options = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        };
        let document =
            roxmltree::Document::parse_with_options(&result.new_pspec_text, options).unwrap();
        let root = document.root_element();
        let first = children(root, "History")
            .next()
            .and_then(|history| children(history, "Update").next())
            .unwrap();
        assert_eq!(first.attribute("release"), Some("4"));
        assert_eq!(
            find_text(first, "Email"),
            "pisi-bump-bot@users.noreply.github.com"
        );
        assert_eq!(find_text(first, "Comment"), "Version bump to 1.94.1");
        assert_eq!(find_text(first, "Date"), RUN_DATE);
    }

    #[test]
    fn should_use_expected_diff_labels_when_brave() {
        let builder = RequestBuilder::new(read_fixture("brave.xml"));
        let result = prepare_update(&builder.request()).unwrap();
        let mut lines = result.unified_diff.lines();
        assert_eq!(lines.next(), Some("--- a/network/browser/brave/pspec.xml"));
        assert_eq!(lines.next(), Some("+++ b/network/browser/brave/pspec.xml"));
    }

    #[test]
    fn should_keep_rest_of_file_byte_identical_when_brave() {
        let text = read_fixture("brave.xml");
        let builder = RequestBuilder::new(text.clone());
        let result = prepare_update(&builder.request()).unwrap();
        let old_lines = split_lines_keep_lf(&text);
        let new_lines = split_lines_keep_lf(&result.new_pspec_text);
        assert_eq!(
            &old_lines[old_lines.len() - 5..],
            &new_lines[new_lines.len() - 5..]
        );
        assert_eq!(new_lines.len(), old_lines.len() + 7);
    }

    #[test]
    fn should_update_only_matching_archive_when_old_url_given() {
        let text = read_fixture("multi-archive.xml");
        let old_url =
            "http://download.brother.com/welcome/dlf101548/hl1210wcupswrapper-3.0.1-1.i386.rpm";
        let builder = RequestBuilder::new(text).old_archive_url(old_url);
        let result = prepare_update(&builder.request()).unwrap();
        let options = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        };
        let trimmed = result.new_pspec_text.trim_start();
        let document = roxmltree::Document::parse_with_options(trimmed, options).unwrap();
        let root = document.root_element();
        let archives = source_archives(root);
        assert!(
            archives[0]
                .text()
                .unwrap()
                .contains("hl1210wlpr-3.0.1-1.i386.rpm")
        );
        assert_eq!(archives[1].attribute("sha1sum"), Some(FAKE_SHA1));
        assert!(
            result
                .warnings
                .contains(&"birden fazla Archive var, sadece eşleşen güncellendi")
        );
    }

    #[test]
    fn should_raise_when_old_url_matches_nothing() {
        let text = read_fixture("multi-archive.xml");
        let builder = RequestBuilder::new(text).old_archive_url("https://nowhere.example/x.zip");
        let error = prepare_update(&builder.request()).unwrap_err();
        assert!(error.to_string().contains("Archive"));
    }

    #[test]
    fn should_update_first_archive_when_old_url_not_given() {
        let text = read_fixture("multi-archive.xml");
        assert!(text.starts_with('\n'));
        let builder = RequestBuilder::new(text);
        let result = prepare_update(&builder.request()).unwrap();
        let options = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        };
        let trimmed = result.new_pspec_text.trim_start();
        let document = roxmltree::Document::parse_with_options(trimmed, options).unwrap();
        let root = document.root_element();
        assert_eq!(
            source_archives(root)[0].attribute("sha1sum"),
            Some(FAKE_SHA1)
        );
    }

    #[test]
    fn should_reproduce_existing_indentation_when_update_is_misindented() {
        let text = read_fixture("odd-indent.xml");
        let builder = RequestBuilder::new(text).new_version("5.6.0");
        let result = prepare_update(&builder.request()).unwrap();
        let added = changed_lines(&result.unified_diff, '+');
        assert!(added.contains(&"            <Update release=\"3\">"));
        assert!(added.contains(&"            <Date>2026-10-07</Date>"));
        assert!(added.contains(&"        </Update>"));
    }

    #[test]
    fn should_not_alter_other_lines_when_misindented() {
        let text = read_fixture("odd-indent.xml");
        let builder = RequestBuilder::new(text);
        let result = prepare_update(&builder.request()).unwrap();
        assert_eq!(changed_lines(&result.unified_diff, '-').len(), 1);
    }

    #[test]
    fn should_warn_when_actions_contains_old_version() {
        let text = read_fixture("atari800.xml");
        let builder = RequestBuilder::new(text).actions_text("WorkDir = \"atari800-5.2.0\"");
        let result = prepare_update(&builder.request()).unwrap();
        assert!(
            result
                .warnings
                .contains(&"actions.py içinde sürüm elle yazılmış, elle kontrol edilmeli")
        );
    }

    #[test]
    fn should_warn_when_actions_contains_underscore_version() {
        let text = read_fixture("atari800.xml");
        let builder = RequestBuilder::new(text).actions_text("tag = '5_2_0'");
        let result = prepare_update(&builder.request()).unwrap();
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn should_not_warn_when_actions_has_no_version() {
        let text = read_fixture("atari800.xml");
        let actions = read_fixture("atari800-actions.py.txt");
        let builder = RequestBuilder::new(text).actions_text(&actions);
        let result = prepare_update(&builder.request()).unwrap();
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn should_warn_and_keep_tabs_when_file_has_tabs() {
        let text = read_fixture("tabs.xml");
        let builder = RequestBuilder::new(text.clone());
        let result = prepare_update(&builder.request()).unwrap();
        assert!(
            result
                .warnings
                .contains(&"dosyada sekme karakteri var (Pisi kuralı: boşluk kullanılmalı)")
        );
        assert_eq!(
            result.new_pspec_text.matches('\t').count(),
            text.matches('\t').count()
        );
    }

    #[test]
    fn should_preserve_crlf_line_endings_when_file_uses_crlf() {
        let text = read_fixture("brave.xml").replace('\n', "\r\n");
        let builder = RequestBuilder::new(text);
        let result = prepare_update(&builder.request()).unwrap();
        assert!(!result.new_pspec_text.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn should_preserve_bom_and_missing_trailing_newline() {
        let text = format!(
            "\u{feff}{}",
            read_fixture("brave.xml").trim_end_matches('\n')
        );
        let builder = RequestBuilder::new(text);
        let result = prepare_update(&builder.request()).unwrap();
        assert!(result.new_pspec_text.starts_with('\u{feff}'));
        assert!(!result.new_pspec_text.ends_with('\n'));
    }

    #[test]
    fn should_escape_ampersand_in_new_url() {
        let text = read_fixture("brave.xml");
        let builder = RequestBuilder::new(text).new_archive_url("https://example.org/a?x=1&y=2");
        let result = prepare_update(&builder.request()).unwrap();
        assert!(result.new_pspec_text.contains("a?x=1&amp;y=2"));
    }

    #[test]
    fn should_not_touch_dependencies_when_updating() {
        let text = read_fixture("brave.xml");
        let builder = RequestBuilder::new(text);
        let result = prepare_update(&builder.request()).unwrap();
        assert!(
            !changed_lines(&result.unified_diff, '+')
                .iter()
                .any(|line| line.contains("Dependency"))
        );
    }

    #[test]
    fn should_raise_when_pspec_is_not_well_formed() {
        let builder = RequestBuilder::new("<PISI><Source>");
        let error = prepare_update(&builder.request()).unwrap_err();
        assert!(error.to_string().contains("okunamadı"));
    }

    #[test]
    fn should_raise_when_history_has_no_update() {
        let text =
            "<PISI><Source><Archive sha1sum=\"a\">u</Archive></Source><History></History></PISI>";
        let builder = RequestBuilder::new(text);
        let error = prepare_update(&builder.request()).unwrap_err();
        assert!(error.to_string().contains("Update"));
    }

    #[test]
    fn should_raise_when_archive_has_no_sha1sum() {
        let text = read_fixture("brave.xml")
            .replace(" sha1sum=\"87f7f1897ed922ec6898b4de3d1297ea2fd1c7ae\"", "");
        let builder = RequestBuilder::new(text);
        let error = prepare_update(&builder.request()).unwrap_err();
        assert!(error.to_string().contains("sha1sum"));
    }
}
