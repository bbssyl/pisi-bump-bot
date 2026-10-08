# ADR-0007: Rewrite pisi-bump-bot in Rust; add a separate nvrs-based source

## Status

Accepted (architecture and feasibility). Implementation is staged: no cutover until parity is proven (see "Phasing").

Date: 2026-10-08. Supersedes the "Python stdlib only" decision for the runtime language. ADR-0006's error-handling principle (catch I/O errors narrowly and turn them into domain errors; let programming errors surface) still applies and is carried over below.

## Context

- The bot today is about 1,930 lines of Python 3 stdlib in 28 modules under `pisi_bump_bot/`, with 190 passing tests and several successful real GitHub Actions runs. It reads Pisi contrib `pspec.xml` recipes, looks up newer GitHub releases, picks a matching release asset, prepares a text-level edit of `pspec.xml` plus a unified diff, builds in Docker (`pisilinux/chroot`) on Actions, classifies failures as permanent or transient (at most 3 attempts), and publishes a Turkish Markdown report and a dashboard issue.
- About 98 of 141 contrib packages have archive URLs that are not on GitHub (SourceForge, vendor sites, GitLab, Launchpad, Apache). The bot reports them as `desteklenmiyor`.
- `nvrs` (crates.io `nvrs`, github.com/koibtw/nvrs, v0.1.10, Rust, edition 2024) checks versions from several sources. The user decided to use it for these packages. That puts Rust tooling into CI either way, so the user also decided to move the whole bot to Rust and drop Python once parity is proven.
- Project rule: everything runs on GitHub Actions only. Nothing is installed on the user's machine.

Binding decisions from the user (not re-opened here): (1) rewrite the bot in Rust and remove Python after parity; (2) ship two separate binaries in one Cargo workspace; (3) run on Actions only; (4) this ADR covers architecture and feasibility only, with no cutover.

## Decision

### 1. Workspace layout

```
pisi-bump-bot/                     (repo root = Cargo workspace root)
  Cargo.toml                       [workspace] members, resolver = "3", shared [workspace.dependencies]
  Cargo.lock                       committed (these are binaries)
  rust-toolchain.toml              pinned toolchain (see Risks / CI)
  crates/
    pisi-bump-common/              lib: recipe model + reading, version normalisation, nvrs-result contract
    pisi-bump-bot/                 bin: port of all current Python behaviour
    pisi-nvrs-source/              bin: thin nvrs wrapper for non-GitHub packages (informational only in v1)
  fixtures/                        moved from tests/fixtures (pspec, assets, state), shared by both crates' tests
```

**A shared `pisi-bump-common` lib crate is justified, but it stays small.** Both binaries need the same three things, and duplicating them would let the two tools drift:

| Shared item | Why both binaries need it |
|---|---|
| `recipe` (port of `spec_parser.py` and `recipes_reader.py`): `PackageRecipe`, `load_recipes`, `newest_update`, `release_number` | `pisi-nvrs-source` has to find the same 141 recipes, skip the same directories (`0oldpackage`, `.git`), and read the same "current version" (the highest `History/Update` release). If this logic differed, the two reports would disagree about the current version. |
| `version` (port of `version_compare.py`): `extract_version_text`, `normalize_version`, `compare_versions` | nvrs returns a raw version string, and "is it newer?" must be decided with the same rules the main bot uses. |
| `nvrs_result` (new): serde types for `nvrs-report.json` | This file is the contract: `pisi-nvrs-source` writes it and `pisi-bump-bot render` reads it. Defining it once makes the contract compile-checked. |

The common crate has no HTTP, async, or CLI dependencies, only `roxmltree`, `regex`, `fancy-regex`, `serde` and `thiserror`, so the main bot does not inherit nvrs's tokio/reqwest stack. Everything else (asset selection, pspec editing, difflib, build state, report rendering) stays inside `pisi-bump-bot`, because only that binary uses it.

### 2. Dependencies

| Concern | Choice | Justification / rejected alternative |
|---|---|---|
| XML | **`roxmltree`** (0.21) | We only parse to read values and to validate after a text-level edit. The edits themselves stay regex/text based, exactly as in Python. roxmltree is a read-only DOM with child iteration, which is all `findall("History/Update")` / `findtext` needs. `quick-xml` is a streaming reader/writer: we would hand-write tree navigation and get no benefit, because we never serialise XML. **Two spike-verified gotchas:** (a) pspec files have `<!DOCTYPE PISI SYSTEM ...>`, and roxmltree rejects it by default (`Err(DtdDetected)`), so `ParsingOptions { allow_dtd: true }` is required. (b) Leading whitespace before `<?xml` is rejected (`UnexpectedDeclaration`), so we must keep Python's `lstrip("﻿ \t\r\n")` (fixture `multi-archive.xml` starts with a blank line). roxmltree only accepts `&str`, so UTF-8 is required (see Risks). |
| Regex | **`regex`** for every pattern without look-around; **`fancy-regex`** only for patterns that need look-around | The Python relies on look-behind and look-ahead in `asset_filters.token`, `asset_selector` (`mask_version`, `ARCHITECTURE_SYNONYMS`), `candidate_url` (`NOT_BEFORE_TOKEN`/`NOT_AFTER_TOKEN`) and `pspec_updater.version_present`. The `regex` crate supports neither look-around nor back-references. Hand-written boundary checks after `find_iter` are rejected because they change `re.sub` semantics on overlapping candidates. `fancy-regex` passes look-around-free sub-expressions to `regex`, so cost is negligible at our input sizes. The single back-reference (`SHA1_ATTRIBUTE` `(["'])...\2`) was rewritten as an alternation (`(?:(")[^"']*"|(')[^"']*')`), which is equivalent because `[^"']` excludes both quotes. The spike verified this. |
| HTTP (main bot) | **`ureq` 3** (blocking, rustls) | Matches the Python's synchronous, one-request-at-a-time model (`GithubLookup` cache, the rate-limit latch, streamed SHA-1). Its dependency tree is small and it needs no async runtime or system OpenSSL. Configure `http_status_as_error(false)` so 4xx/5xx responses come back as responses, mirroring `read_error_response`. Python's 60 s timeout applies per socket operation, not to the whole request, so use connect and response timeouts and do **not** set a global body timeout (archives can be 2 GB). `reqwest` was rejected for the main bot because it brings tokio, and nothing in the main bot is concurrent. |
| HTTP (nvrs binary) | **`reqwest` 0.13** (whatever nvrs pins) + `tokio` | Not a free choice. `nvrs::run_source` takes a `reqwest::Client` and is `async`, so this binary must depend on the same reqwest semver line. reqwest 0.13's `default-tls` is rustls on aws-lc-rs (spike: no system libssl linked; `aws-lc-sys` needs a C compiler, which Ubuntu runners have). |
| SHA-1 | **`sha1`** (RustCrypto), streaming `Digest::update` over 1 MiB chunks | Direct equivalent of `hashlib.sha1()` streaming. Hex output is formatted by hand, so no `hex` crate is needed. |
| JSON | **`serde` + `serde_json`** (typed structs; `BTreeMap` for the state `packages` map, which Python writes with sorted keys) | Spike: re-serialising the real `report.json` (66 KB, Turkish text) and `state/builds.json` with `to_string_pretty` + `"\n"` is **byte-identical** to Python's `json.dumps(indent=2, ensure_ascii=False)`. |
| CLI | **`clap` v4 derive**, subcommands `report`, `prepare`, `merge-results`, `render` | Flags mirror the argparse ones. One deliberate change: the Python "bare" report form (`python3 -m pisi_bump_bot --recipes-dir ...`) becomes an explicit `report` subcommand, since the workflow is rewritten at cutover anyway. clap's usage-error exit code (2) matches argparse. |
| Unified diff and similarity | **Own port of `difflib`** (`SequenceMatcher` with autojunk, `get_matching_blocks`, `get_opcodes`, `get_grouped_opcodes`, `unified_diff`): about 170 lines in `pisi-bump-bot::difflib` | `SequenceMatcher.ratio()` drives asset selection (`MIN_SIMILARITY = 0.6`, `MIN_MARGIN = 0.05`), so it must be ported exactly whatever we do for diffs. Spike evidence against `similar` 3.x: it matched all 5 pspec diffs but **failed the 280-line diff** (difflib's autojunk heuristic is active at 200 or more lines), and its `ratio()` **disagreed with difflib on 3,270 of 6,229 string pairs**. The port matched on all of them. One implementation serves both needs and `similar` is dropped. |
| Date | **`time`** (0.3, default features) | Only "today in UTC as YYYY-MM-DD" is needed (`OffsetDateTime::now_utc().date()` displays exactly that). `chrono` is larger and offers time zone and formatting features we don't use. |
| xz | **`lzma-rs`** (pure Rust) for `pisi-index.xml.xz` | No system liblzma. Throughput is irrelevant for one index file. Fallback: the `liblzma` crate (static C) if lzma-rs fails on the real index. Verify in the port wave. |
| Percent-encoding | **`percent-encoding`** with an explicit `AsciiSet` replicating Python `quote(safe="/")` (unreserved `A-Za-z0-9_.-~` plus `/`), and `percent_decode_str(..).decode_utf8_lossy()` for `unquote` | `candidate_choice` builds release-download URLs with these. The safe set must match exactly, or asset URLs will differ. |
| Errors | **`thiserror`**: one enum per module (`FetchError`, `UpstreamError`, `RateLimitExceeded`, `PspecUpdateError`, `ArchiveDownloadError { transient: bool }`, `PrepareFailure { transient }`, `StateError`, `ReportLoadError`) | No `anyhow` inside the library modules. Only `main` maps domain errors to stderr text and an exit code, as `cli.run` / `run_subcommand` do. |
| Directory walk | `std::fs::read_dir` recursion (about 15 lines) | Matches `os.walk`: don't follow symlinks (use `DirEntry::file_type`, not `metadata`), skip unreadable directories silently, prune `0oldpackage` and `.git`, then sort. No `walkdir` dependency. |

**The Rust form of ADR-0006's "narrow catch".** Python `except (OSError, ValueError) as e: raise DomainError(...) from e` becomes `.map_err(|source| DomainError::Variant { source })` on the **specific** fallible call (`std::io::Error`, `serde_json::Error`, `roxmltree::Error`, `ureq::Error::Io`/`Timeout`), with the source kept via `#[source]`/`#[from]`. Callers `match` on the exact variants they handle (e.g. `PackageChecker` handles `RateLimitExceeded` and `UpstreamError`, but **not** `FetchError`, which is converted inside `GithubLookup`, exactly as in Python). Every other variant propagates with `?`. Programming errors (failed static regex compile, broken invariants) are `expect("…")` panics and are never caught. There are no blanket `impl From<E> for BotError` conversions that would silently widen what a caller handles, no `catch_unwind` in the main bot, and no `Box<dyn Error>` in module signatures.

### 3. `pisi-nvrs-source`: use nvrs as a library, behind guard rails

The spike (details in "Feasibility evidence") showed the public library API is usable for checking one package at a time, with no config file, no state file and no CLI:

```rust
let package: nvrs::config::Package =
    serde_json::from_value(json!({"source": "regex", "url": url, "regex": pattern}))?;
let task = tokio::spawn(nvrs::run_source((name, package), client.clone(), None));
match task.await {
    Ok(Ok(release)) => /* release.name is the version string */,
    Ok(Err(nvrs_error)) => /* per-package informational error */,
    Err(join_error) if join_error.is_panic() => /* per-package error: nvrs panicked */,
}
```

Shelling out to the `nvrs` binary was rejected. It would need the `cli` feature, a generated TOML file, oldver/newver JSON state files, and output parsing, all to reach the same `run_source` call.

Guard rails, all required because of what the spike found:

1. **Build `Package` through serde, not `Package::new`.** `Package::new("regex", url, ..)` sets only `url` and leaves `regex` empty (spike: `get_api()` returns `("regex", [url, ""])`), and the `regex` field is private with no setter. `Package` derives `Deserialize`, so `serde_json::from_value` sets every field.
2. **Isolate every check in `tokio::spawn`.** nvrs `unwrap()`s internally: an invalid regex panics (`Regex::new(..).unwrap()`), a regex without capture group 1 panics (`caps.get(1).unwrap()`), and `use_max_tag` on an empty tag list panics. The spike confirmed `tokio::spawn` turns these into `JoinError::is_panic()` for that package only. **Never set `panic = "abort"`** in the workspace release profile, or this isolation stops working. nvrs's own `panic = "abort"` applies only when building nvrs itself, not when it is a dependency.
3. **Pre-validate generated regexes** with the `regex` crate before calling nvrs: the pattern must compile and have `captures_len() >= 2`.
4. **Narrow mapping of nvrs errors.** `RequestError`, `RequestNotOK`, `RequestForbidden` and `NoVersion` become per-package informational errors. `SourceNotFound` means our mapping table names a source that isn't compiled in, so it is a programming or config error and fails the run. nvrs's 403 message says "request returned 430", so we write our own Turkish text from the variant and never forward nvrs's message.
5. **Use our own `reqwest::Client`** with a timeout and a `pisi-bump-bot` User-Agent. nvrs itself sends `User-Agent: nvrs` per request. Run checks with bounded concurrency (a `tokio::sync::Semaphore` of 4) to stay polite.
6. **Pin `nvrs = "=0.1.10"`** with `Cargo.lock` committed. The crate describes itself as "still a WIP", it is pre-1.0 with a single maintainer, and its API can break in any minor release.

**Feature set:** `nvrs = { version = "=0.1.10", default-features = false, features = ["regex", "gitlab", "github"] }`. Add `"gitea"` only if a contrib package turns out to be hosted on Codeberg or another Gitea instance.

| Feature | Enable? | Reason |
|---|---|---|
| `regex` | yes | This is the generic "website" check: GET a URL and take the **first** match of capture group 1. It covers SourceForge, JetBrains (nvrs's own test uses `data.services.jetbrains.com`), Launchpad, Apache, and vendor download pages. |
| `gitlab` | yes | gitlab.com and self-hosted hosts (`host` field). |
| `github` | yes | Cheap to include, and useful as a cross-check against the main bot. Not used for the 98 non-GitHub packages. |
| `shell` | **no** | Runs `sh -c <string>`. Our config strings are derived from pspec URLs, which is an injection risk, and we gain nothing. |
| `aur` | no (v1) | It returns Arch packaging versions, which are downstream information, not upstream releases. Can be revisited as a hint source. |
| `crates-io` | no | No contrib package is a Rust crate release. |
| `cli` | no | Library use only. |

**There is no SourceForge source in nvrs.** The source modules are `aur`, `crates_io`, `gitea`, `github`, `gitlab`, `regex` and `shell`, and `API_LIST` with our features is `["github", "gitlab", "regex"]`. SourceForge must go through `regex` against a SourceForge page (the project RSS feed or files listing). Because nvrs's regex source returns the **first** match, not the highest version, the page we target must list the newest release first. The nvrs-source wave must check this against real pages (from Actions). Our side then re-checks "newer?" with `pisi-bump-common::version`.

**How packages map to sources:** derived automatically from the archive URL where the URL pattern allows it (gitlab.com and other GitLab hosts become `gitlab`; `sourceforge.net/projects/<p>/` becomes `regex` with a generated RSS URL and a pattern built from the old file name with the version masked). Vendor sites go in a hand-maintained `nvrs-sources.toml` in the repo (URL + regex per package). Some vendors cannot be covered by nvrs at all. For example, a Discord-style `api/download?platform=linux` link carries the version only in the redirect `Location` header, and nvrs's regex source only sees the response body. These stay `desteklenmiyor` with a reason. **v1 is informational only:** the nvrs result never changes a package's `Status`. `render` adds a separate "Diğer kaynaklar (nvrs, bilgi amaçlı)" section from `nvrs-report.json`, and `prepare` keeps selecting only `eski` packages, so nvrs findings cannot reach prepare or build.

## Feasibility evidence (spike, scratch only, not committed)

Location: `/tmp/claude-1000/rust-spike/` (`pspec-spike/`, `nvrs-spike/`, `ref/`). Toolchain: rustc/cargo 1.99.0.

### A. Text-level pspec edit: byte-identical to Python

A Rust port of `prepare_update` (regex Archive replace, sha1 attribute rewrite, History/Update insertion with detected indentation and newline, roxmltree validation, warnings, difflib-port unified diff) was compared against fresh Python reference output from the same inputs (version, URL, `sha1=0123456789abcdef0123456789abcdef01234567`, `run_date=2026-10-08`):

| Case | What it exercises | new pspec.xml | pspec.diff | `similar` crate diff |
|---|---|---|---|---|
| `brave.xml` | DOCTYPE, multi-line Archive text with surrounding whitespace | identical | identical | identical |
| `odd-indent.xml` | first `<Update>` indented 12 spaces with closing tag at 8; URL containing `&` (escaped to `&amp;`) | identical | identical | identical |
| `multi-archive.xml` | leading blank line before `<?xml`; matching the 2nd Archive by `old_archive_url`; warning text | identical | identical | identical |
| `tabs.xml` | tab warning | identical | identical | identical |
| `brave.xml` with BOM and CRLF (generated) | BOM strip for parsing; CRLF detection for the new block; `\r` kept inside diff lines | identical | identical | identical |
| 280-line Markdown, 25 random edits | difflib autojunk (200 lines or more) | n/a | identical | **different** |
| 6,229 string pairs (asset fixtures × asset fixtures, plus 30 random 150–400 character strings) | `SequenceMatcher.ratio()` | n/a | 0 mismatches (exact `f64` equality) | **3,270 mismatches** |

Example: the odd-indent diff, identical byte for byte from Python and Rust:

```diff
--- a/editor/jedit/pspec.xml
+++ b/editor/jedit/pspec.xml
@@ -18,7 +18,7 @@
             <Dependency>jdk8-openjdk</Dependency>
             <Dependency>jre8-openjdk</Dependency>
         </BuildDependencies>
-        <Archive sha1sum="91a0537243e09ecbc273fbb78ce6d07b37f2131e" type="binary">https://sourceforge.net/projects/jedit/files/jedit/5.5.0/jedit5.5.0install.jar</Archive>
+        <Archive sha1sum="0123456789abcdef0123456789abcdef01234567" type="binary">https://sourceforge.net/projects/jedit/files/jedit/5.6.0/jedit5.6.0install.jar?a=1&amp;b=2</Archive>
     </Source>
     <Package>
         <Name>jedit</Name>
@@ -43,6 +43,13 @@
     </Package>
 
     <History>
+            <Update release="3">
+            <Date>2026-10-08</Date>
+            <Version>5.6.0</Version>
+            <Comment>Version bump to 5.6.0</Comment>
+            <Name>pisi-bump-bot</Name>
+            <Email>pisi-bump-bot@users.noreply.github.com</Email>
+        </Update>
             <Update release="2">
             <Date>2020-02-02</Date>
             <Version>5.5.0</Version>
```

The behaviours that must be preserved, and that the spike reproduced: (1) Archive regex `(<Archive\b[^>]*>)(\s*)([^<]*?)(\s*)(</Archive>)` keeps the surrounding whitespace groups 2 and 4 and swaps only group 3. The old URL is compared after XML-unescaping and the new URL is XML-escaped (only `& < >`, like `saxutils.escape`). (2) The sha1sum value is replaced inside the opening tag, keeping its quote style. (3) The new `<Update release="max+1">` goes before the first `<Update` inside `<History>`, at the start of that line. The opening indent comes from that line's prefix (an error if it isn't pure whitespace). Each child's indent is copied from the same tag in the existing block, falling back to opening indent plus 4 spaces. The closing indent is copied from the existing `</Update>`. The newline is `\r\n` if the existing block contains one. (4) The result is re-parsed and checked: the newest release equals the new release and version, and an Archive with the new sha1 and URL exists. (5) The diff splits on `\n` only (so `\r` stays in the line), uses n=3, headers `a/<dir>/pspec.xml` and `b/<dir>/pspec.xml` without dates, and appends `\n` to a last line that lacks one, with no "No newline" marker.

### B. nvrs: usable as a library (with the guard rails above)

The scratch crate depended on `nvrs` 0.1.10 from crates.io (its `lib.rs`, `api/regex.rs` and `config.rs` are identical to GitHub `main`) with `default-features = false, features = ["regex","gitlab","github"]`. It called `run_source` against a **loopback** HTTP server serving a fake SourceForge-style page. No external network was used.

| # | Scenario | Result |
|---|---|---|
| 1 | regex source, `jedit(\d+(?:\.\d+)+)install\.jar` | `ok version="5.7.0"` (first match on the page) |
| 2 | HTTP 404 | `Err(RequestNotOK)`: "jedit: request status != OK, 404 Not Found" |
| 3 | no match | `Err(NoVersion)`: "jedit: version not found" |
| 4 | regex without capture group | **panic** inside nvrs, contained by `tokio::spawn` (`is_panic=true`) |
| 5 | invalid regex | **panic** inside nvrs (`Regex::new(..).unwrap()`), contained by `tokio::spawn` |
| 6 | `Package::new("regex", url, ..)` | `get_api() = ("regex", [url, ""])`, so the regex can't be set this way; use serde |
| 7 | `source = "shell"` with the feature off | `Err(SourceNotFound)` |
| 8 | `API_LIST` | `["github", "gitlab", "regex"]` |

Size and cost: the nvrs spike binary is 9.7 MB in release (about 130 crates: tokio, hyper, rustls, aws-lc-sys). The pspec spike binary is 4.1 MB. Highest dependency MSRV seen: 1.88 (nvrs tree, via `icu_*`), and 1.85 for the main-bot tree.

## Rationale

- **One language.** nvrs forces Rust into CI, and keeping Python too would mean two toolchains, two test runners and two dependency stories.
- **Two binaries.** They make the trusted, build-capable path (`pisi-bump-bot`) visibly separate from the informational, heuristic path (`pisi-nvrs-source`). Each also keeps its own dependency tree: blocking ureq vs tokio/reqwest.
- **Parity is achievable.** The parts most likely to drift (formatting-preserving text edits, difflib output, similarity scores, JSON formatting) were checked byte for byte against Python output in the spike.
- **Porting difflib ourselves** (about 170 lines) is safer than adopting a diff crate whose algorithm differs: the asset selection thresholds were tuned against difflib's numbers.

## Consequences

Positive:
- One toolchain. Static binaries with no Python on the runner. Asset selection, pspec editing and failure policy all carry over unchanged.
- The `nvrs-report.json` contract is typed in `pisi-bump-common`, so the two tools cannot silently disagree on the format.

Negative / costs:
- A full port of about 1,900 lines plus about 190 tests, then a parity phase before Python can be deleted.
- We now own a `difflib` port and must keep its tests.
- The nvrs binary brings in a large async and TLS dependency tree (aws-lc-sys C build), so cold CI builds take minutes and `Swatinem/rust-cache` is needed.
- We depend on a pre-1.0, single-maintainer crate with internal `unwrap()`s. This is mitigated by the exact pin, panic isolation and regex pre-validation. Exit plan: nvrs's `regex` source is about 20 lines, so if nvrs stalls we can replace it in-house without changing the contract.

Neutral:
- The CLI gets an explicit `report` subcommand.
- Edge cases where Rust deliberately differs from Python (listed under Risks) are documented and covered by tests, not hidden.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Keep Python, add only the Rust nvrs tool | Decided against by the user (one-language goal). Would also leave two toolchains in CI. |
| Shell out to the `nvrs` CLI | Needs the `cli` feature, a generated TOML file, oldver/newver JSON files and output scraping, all to reach the same `run_source` that the library exposes directly. |
| Single binary with an `nvrs` subcommand | Blurs the trusted and informational boundary the user asked for, and links tokio/reqwest into the main bot. |
| No common crate (duplicate the code) | Recipe reading and version comparison would drift between the two tools, and the result-file contract would be untyped. |
| `quick-xml` | Streaming API. We'd rebuild tree navigation by hand for no gain, since we never write XML. |
| `similar` for diffs and ratio | Spike: differs from difflib on large diffs (autojunk) and on 52% of ratio pairs. |
| `reqwest` in the main bot | Async runtime with no concurrency need. |
| `chrono` | Larger than needed for one ISO date. |

## Risks (honest list)

1. **CI toolchain drift.** This is the same class of bug as the earlier Python 3.12 vs 3.14 mismatch (LP-20261007-ci-python-version-drift). The local toolchain is 1.99.0, while `ubuntu-latest` ships whatever rustup stable the image was built with, and edition 2024 plus the dependency MSRVs (1.85 and 1.88 above) set a floor. Mitigation: commit `rust-toolchain.toml` with `channel = "1.99.0"` and `components = ["clippy", "rustfmt"]`. rustup on the runner honours it automatically, and a `dtolnay/rust-toolchain` step can read it. Also commit `Cargo.lock`, and print `rustc -Vv` in the workflow log. The later workflow-rewrite task must check this on a real Actions run before trusting it.
2. **Non-UTF-8 pspec files.** Python parses recipe **bytes** and honours an `encoding=` declaration. roxmltree needs UTF-8 `&str`. A latin-1 or iso-8859-9 pspec would become "pspec.xml okunamadı" in Rust. Mitigation: the parity run must compare the unreadable-path lists. If any exist, decode with `encoding_rs` from the declaration.
3. **Unicode semantics.** Python `\d` + `int()` accepts non-ASCII digits and is unbounded. Rust `u64` parsing is not. Decision: compare numeric segments as digit strings (strip leading zeros, then compare length, then lexically), which gives exact Python ordering with no overflow, and restrict to ASCII digits (a documented, deliberate difference). Similarly `casefold()` vs `to_lowercase()` in `sort_packages` (only differs for characters like `ß`; package names are ASCII), `str.strip()` whitespace (Python also strips `\x1c`–`\x1f`), and `str.isdigit()` in `parse_release` (Python crashes on `"²"`; Rust returns 0, which is intentionally safer).
4. **`html.unescape`** in `matches_old_url` is replaced with XML entity decoding (`&amp; &lt; &gt; &quot; &apos;` plus numeric `&#..;`). Equivalent for well-formed XML; HTML-only entities would already fail the XML parse.
5. **ureq timeout semantics** differ from urllib's per-socket timeout. A misconfigured global timeout would break 2 GB downloads. Needs a loopback test (port `test_archive_download_loopback.py`).
6. **nvrs specifics:** first-match (not max-version) regex semantics; internal panics; a pre-1.0 API; SourceForge only through `regex`; vendor redirect-only URLs that can't be covered. All are contained by the guard rails, and none can affect prepare or build in v1.
7. **Build time and size** on Actions (aws-lc-sys, about 130 crates for the nvrs binary). Mitigation: rust-cache, and building both binaries once per workflow and passing them on as an artifact.
8. **Parity coverage.** The spike checked the hardest pieces, not everything. Cutover requires the differential CI job described below.

## Phasing

1. **Wave 1 (T2, parallel):** `pisi-bump-common` (recipe, version, nvrs_result) and `pisi-bump-bot::difflib` (port from this spike, with tests reused from the spike's reference generation).
2. **Wave 2 (T2/T3):** the remaining main-bot modules (table below), porting the 190 Python tests and reusing the fixtures unchanged.
3. **Wave 3:** `pisi-nvrs-source` (URL-to-source mapping, `nvrs-sources.toml`, `nvrs-report.json`), plus a `render` section for it.
4. **Parity phase (Actions only):** a job runs Python and Rust on the same contrib commit and recorded fixtures (`report.json`, `REPORT.md`, `new-updates`, `prepare` output tree, state JSON, merge comment) and `diff`s every output. Network results are pinned by recording the GitHub responses once.
5. **Cutover (separate task, needs user approval):** rewrite the workflows to call the binaries, then delete `pisi_bump_bot/` and `setup-python`.

## Module port plan

| Python file | Rust module | Crate / binary | Complexity / risk |
|---|---|---|---|
| `errors.py` | per-module `thiserror` enums | both | Low |
| `spec_parser.py` | `recipe::spec` | common | Low (roxmltree `allow_dtd`) |
| `recipes_reader.py` | `recipe::reader` | common | Medium (walk semantics, BOM/whitespace strip, UTF-8 risk) |
| `version_compare.py` | `version` | common | Medium (digit-string compare, prerelease regex) |
| *(new)* | `nvrs_result` | common | Low |
| `text_files.py` | `text_files` | bot | Low (read bytes, UTF-8, no newline translation) |
| `http_client.py` | `http` (ureq agent, `stream_sha1`) | bot | Medium (status-as-response, timeouts) |
| `github_upstream.py` | `github` (regex table, `GithubLookup` cache + rate-limit latch) | bot | Medium |
| `candidate_url.py` | `candidate_url` | bot | Medium (fancy-regex look-around) |
| `asset_filters.py` | `asset_filters` | bot | Medium (fancy-regex `token()` patterns) |
| `asset_selector.py` | `asset_selector` | bot | **High** (ratio parity, product-stem guard, version masking, arch synonyms, refusal text) |
| `candidate_choice.py` | `candidate_choice` | bot | Medium (`quote`/`unquote` safe set) |
| `package_checker.py` | `package_checker` | bot | Low–Medium (narrow error matching) |
| `index_consistency.py` | `index_consistency` | bot | Medium (xz via lzma-rs, large XML) |
| `report_model.py` | `report::model` (serde) | bot | Low |
| `report_writer.py` | `report::writer` | bot | Medium (Markdown and JSON byte parity, casefold sort) |
| `report_loader.py` | `report::loader` | bot | Low |
| `new_updates.py` | `report::new_updates` | bot | Low |
| `pspec_updater.py` | `pspec_updater` | bot | **High**, but de-risked by the spike (byte-identical on 5 cases) |
| *(difflib stdlib)* | `difflib` | bot | **High**, but de-risked by the spike (0/6,229 ratio mismatches, autojunk diff identical) |
| `archive_download.py` | `archive_download` | bot | Medium (size cap, transient classification, loopback test) |
| `build_state.py` | `build::state` | bot | Low (BTreeMap, legacy `logic_version` default) |
| `failure_policy.py` | `build::failure_policy` | bot | Low |
| `prepare_queue.py` | `build::queue` | bot | Low |
| `prepare_runner.py` | `prepare` | bot | **High** (pruning and empty-parent removal, expiry, ordering, file writes without newline translation) |
| `build_results.py` | `build::results` | bot | Low–Medium (rglob order, lenient parsing) |
| `build_columns.py` | `report::build_columns` | bot | Low |
| `subcommands.py`, `cli.py`, `__main__.py` | `main.rs` (clap) | bot | Low–Medium (exit codes, stderr messages) |
| *(new)* | `main.rs`, `mapping`, `checks` | `pisi-nvrs-source` | Medium–High (source mapping, real-page validation on Actions) |
