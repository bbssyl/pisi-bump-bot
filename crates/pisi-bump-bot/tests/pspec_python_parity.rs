use std::fs;
use std::path::PathBuf;

use pisi_bump_bot::pspec_updater::{UpdateRequest, prepare_update};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/pspec")
}

const EXPECTED_PYTHON_DIFF: &str = "--- a/editor/jedit/pspec.xml
+++ b/editor/jedit/pspec.xml
@@ -18,7 +18,7 @@
             <Dependency>jdk8-openjdk</Dependency>
             <Dependency>jre8-openjdk</Dependency>
         </BuildDependencies>
-        <Archive sha1sum=\"91a0537243e09ecbc273fbb78ce6d07b37f2131e\" type=\"binary\">https://sourceforge.net/projects/jedit/files/jedit/5.5.0/jedit5.5.0install.jar</Archive>
+        <Archive sha1sum=\"0123456789abcdef0123456789abcdef01234567\" type=\"binary\">https://example.org/new-1.94.1.zip</Archive>
     </Source>
     <Package>
         <Name>jedit</Name>
@@ -43,6 +43,13 @@
     </Package>
\u{20}
     <History>
+            <Update release=\"3\">
+            <Date>2026-10-07</Date>
+            <Version>5.6.0</Version>
+            <Comment>Version bump to 5.6.0</Comment>
+            <Name>pisi-bump-bot</Name>
+            <Email>pisi-bump-bot@users.noreply.github.com</Email>
+        </Update>
             <Update release=\"2\">
             <Date>2020-02-02</Date>
             <Version>5.5.0</Version>
";

#[test]
fn should_match_fresh_python_output_byte_for_byte_on_odd_indent() {
    let pspec_text = fs::read_to_string(fixtures_dir().join("odd-indent.xml")).unwrap();
    let request = UpdateRequest {
        recipe_path: "editor/jedit",
        pspec_text: &pspec_text,
        actions_text: None,
        new_version: "5.6.0",
        new_archive_url: "https://example.org/new-1.94.1.zip",
        new_sha1: "0123456789abcdef0123456789abcdef01234567",
        run_date: "2026-10-07",
        old_archive_url: None,
    };
    let result = prepare_update(&request).unwrap();

    assert_eq!(
        result.unified_diff, EXPECTED_PYTHON_DIFF,
        "Rust output must be byte-identical to fresh Python `prepare_update` output for the same inputs (captured via `python3 -I` against the live pisi_bump_bot module on 2026-10-08)"
    );
}
