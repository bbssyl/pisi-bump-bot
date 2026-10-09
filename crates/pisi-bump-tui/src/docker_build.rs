use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

use crate::app::WorkerMessage;

const BUILD_SCRIPT: &str = include_str!("../../../ci/pisi-build-in-container.sh");
const IMAGE: &str = "pisilinux/chroot";

pub const DOCKER_NOT_INSTALLED_MESSAGE: &str = "Docker kurulu değil.";
pub const DOCKER_DAEMON_UNREACHABLE_MESSAGE: &str = "Docker servisi çalışmıyor.";
pub const DOCKER_PERMISSION_DENIED_MESSAGE: &str = "Docker soketine erişim izniniz yok.";
pub const DOCKER_PERMISSION_FIX_COMMAND: &str = "sudo usermod -aG docker $USER";
pub const SUDO_MISSING_MESSAGE: &str = "sudo bulunamadı. Kalıcı çözüm: kullanıcınızı docker grubuna ekleyin (sudo usermod -aG docker $USER) ya da sisteminizin yetkilendirme aracını (sudo, doas vb.) kurun.";
pub const WRONG_PASSWORD_MESSAGE: &str = "Yanlış şifre, tekrar deneyin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockerAvailability {
    Available,
    NotInstalled,
    DaemonUnreachable,
    PermissionDenied,
}

pub fn classify_docker_output(success: bool, stderr: &str) -> DockerAvailability {
    if success {
        DockerAvailability::Available
    } else if stderr.to_lowercase().contains("permission denied") {
        DockerAvailability::PermissionDenied
    } else {
        DockerAvailability::DaemonUnreachable
    }
}

pub fn docker_status_message(availability: DockerAvailability) -> Option<String> {
    match availability {
        DockerAvailability::Available => None,
        DockerAvailability::NotInstalled => Some(DOCKER_NOT_INSTALLED_MESSAGE.to_string()),
        DockerAvailability::DaemonUnreachable => {
            Some(DOCKER_DAEMON_UNREACHABLE_MESSAGE.to_string())
        }
        DockerAvailability::PermissionDenied => Some(permission_denied_message()),
    }
}

pub fn permission_denied_message() -> String {
    format!(
        "{DOCKER_PERMISSION_DENIED_MESSAGE}\nKalıcı çözüm: {DOCKER_PERMISSION_FIX_COMMAND}\n(ardından oturumu yeniden açmanız gerekir)"
    )
}

fn check_docker() -> DockerAvailability {
    match Command::new("docker").arg("info").output() {
        Err(_) => DockerAvailability::NotInstalled,
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            classify_docker_output(output.status.success(), &stderr)
        }
    }
}

fn sudo_available() -> bool {
    Command::new("sudo").arg("-V").output().is_ok()
}

fn is_wrong_password(stderr: &str) -> bool {
    let lowered = stderr.to_lowercase();
    lowered.contains("incorrect password")
        || lowered.contains("try again")
        || lowered.contains("sorry")
}

fn write_password(child: &mut std::process::Child, password: &str) -> Result<(), String> {
    let Some(stdin) = child.stdin.as_mut() else {
        return Err("sudo stdin açılamadı".to_string());
    };
    stdin
        .write_all(password.as_bytes())
        .and_then(|()| stdin.write_all(b"\n"))
        .map_err(|error| format!("sudo'ya şifre iletilemedi: {error}"))
}

fn run_docker_sudo_capturing(password: &str, args: &[&str]) -> Result<Output, String> {
    let mut child = Command::new("sudo")
        .arg("-S")
        .arg("docker")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("sudo başlatılamadı: {error}"))?;
    write_password(&mut child, password)?;
    child
        .wait_with_output()
        .map_err(|error| format!("sudo çalıştırılamadı: {error}"))
}

fn docker_available_sudo(password: &str) -> Result<(), String> {
    let output = run_docker_sudo_capturing(password, &["info"])?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_wrong_password(&stderr) {
        Err(WRONG_PASSWORD_MESSAGE.to_string())
    } else {
        Err(format!("sudo docker info başarısız: {}", stderr.trim()))
    }
}

fn unique_temp_dir(label: &str) -> std::io::Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("pisi-bump-tui-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn materialize_script() -> std::io::Result<PathBuf> {
    let dir = unique_temp_dir("ci")?;
    let script_path = dir.join("pisi-build-in-container.sh");
    std::fs::write(&script_path, BUILD_SCRIPT)?;
    set_executable(&script_path)?;
    Ok(dir)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

const PSPEC_FILE_SUFFIX: &str = "/pspec.xml";
const BUILDS_DIR_NAME: &str = "pisi-bump-tui-builds";

fn recipe_directory_path(recipe_path: &str) -> &str {
    recipe_path
        .strip_suffix(PSPEC_FILE_SUFFIX)
        .unwrap_or(recipe_path)
}

fn build_output_path(recipes_dir: &Path, recipe_path: &str) -> PathBuf {
    let directory = recipe_directory_path(recipe_path);
    let base = recipes_dir.parent().unwrap_or(recipes_dir);
    base.join(BUILDS_DIR_NAME).join(directory)
}

fn build_output_dir(recipes_dir: &Path, recipe_path: &str) -> std::io::Result<PathBuf> {
    let path = build_output_path(recipes_dir, recipe_path);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn docker_run_args(
    recipes_dir: &Path,
    recipe_path: &str,
    script_dir: &Path,
    output_dir: &Path,
) -> Vec<String> {
    vec![
        "run".to_string(),
        "--rm".to_string(),
        "--security-opt".to_string(),
        "seccomp=unconfined".to_string(),
        "-v".to_string(),
        format!("{}:/git", recipes_dir.display()),
        "-v".to_string(),
        format!("{}:/out", output_dir.display()),
        "-v".to_string(),
        format!("{}:/ci:ro", script_dir.display()),
        IMAGE.to_string(),
        "bash".to_string(),
        "/ci/pisi-build-in-container.sh".to_string(),
        recipe_directory_path(recipe_path).to_string(),
        "false".to_string(),
    ]
}

fn pull_image(sudo_password: Option<&str>) -> Result<(), String> {
    match sudo_password {
        None => {
            let status = Command::new("docker").args(["pull", IMAGE]).status();
            if matches!(status, Ok(exit_status) if exit_status.success()) {
                Ok(())
            } else {
                Err("docker pull başarısız oldu".to_string())
            }
        }
        Some(password) => {
            let output = run_docker_sudo_capturing(password, &["pull", IMAGE])?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "sudo docker pull başarısız: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ))
            }
        }
    }
}

fn run_build_with_runner(
    recipes_dir: &Path,
    recipe_path: &str,
    sudo_password: Option<&str>,
) -> (bool, String, Option<PathBuf>) {
    let script_dir = match materialize_script() {
        Ok(dir) => dir,
        Err(error) => {
            return (
                false,
                format!("derleme betiği hazırlanamadı: {error}"),
                None,
            );
        }
    };
    let output_dir = match build_output_dir(recipes_dir, recipe_path) {
        Ok(dir) => dir,
        Err(error) => {
            return (false, format!("çıktı dizini oluşturulamadı: {error}"), None);
        }
    };
    let args = docker_run_args(recipes_dir, recipe_path, &script_dir, &output_dir);

    let success = match sudo_password {
        None => Command::new("docker")
            .args(&args)
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        Some(password) => {
            let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
            run_docker_sudo_capturing(password, &arg_refs)
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
    };

    let log = std::fs::read_to_string(output_dir.join("build.log"))
        .map(|raw| sanitize_carriage_return_progress(&raw))
        .unwrap_or_else(|_| "build.log okunamadı".to_string());
    (success, log, Some(output_dir))
}

fn sanitize_carriage_return_progress(text: &str) -> String {
    text.split('\n')
        .map(|chunk| match chunk.rsplit_once('\r') {
            Some((_, last_frame)) => last_frame,
            None => chunk,
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

fn proceed_build(
    tx: &Sender<WorkerMessage>,
    row_index: usize,
    recipes_dir: &Path,
    recipe_path: &str,
    sudo_password: Option<&str>,
) {
    let _ = tx.send(WorkerMessage::BuildPulling { row_index });
    if let Err(reason) = pull_image(sudo_password) {
        let _ = tx.send(WorkerMessage::BuildComplete {
            row_index,
            success: false,
            log: reason,
            output_dir: None,
        });
        return;
    }

    let _ = tx.send(WorkerMessage::BuildRunning { row_index });
    let (success, log, output_dir) = run_build_with_runner(recipes_dir, recipe_path, sudo_password);
    let _ = tx.send(WorkerMessage::BuildComplete {
        row_index,
        success,
        log,
        output_dir,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildCheckOutcome {
    Proceed,
    OfferSudoRetry(String),
    Blocked(String),
}

pub fn decide_build_check(
    availability: DockerAvailability,
    sudo_is_available: bool,
) -> BuildCheckOutcome {
    match availability {
        DockerAvailability::Available => BuildCheckOutcome::Proceed,
        DockerAvailability::PermissionDenied => {
            if sudo_is_available {
                BuildCheckOutcome::OfferSudoRetry(permission_denied_message())
            } else {
                BuildCheckOutcome::Blocked(SUDO_MISSING_MESSAGE.to_string())
            }
        }
        other => BuildCheckOutcome::Blocked(docker_status_message(other).unwrap_or_default()),
    }
}

pub fn spawn_build(
    tx: Sender<WorkerMessage>,
    row_index: usize,
    recipes_dir: PathBuf,
    recipe_path: String,
) {
    thread::spawn(
        move || match decide_build_check(check_docker(), sudo_available()) {
            BuildCheckOutcome::Proceed => {
                proceed_build(&tx, row_index, &recipes_dir, &recipe_path, None);
            }
            BuildCheckOutcome::OfferSudoRetry(reason) => {
                let _ = tx.send(WorkerMessage::BuildPermissionDenied { row_index, reason });
            }
            BuildCheckOutcome::Blocked(reason) => {
                let _ = tx.send(WorkerMessage::BuildDockerUnavailable { row_index, reason });
            }
        },
    );
}

pub fn spawn_sudo_build(
    tx: Sender<WorkerMessage>,
    row_index: usize,
    recipes_dir: PathBuf,
    recipe_path: String,
    password: String,
) {
    thread::spawn(move || {
        if let Err(reason) = docker_available_sudo(&password) {
            let _ = tx.send(WorkerMessage::BuildSudoRejected { row_index, reason });
            return;
        }
        proceed_build(&tx, row_index, &recipes_dir, &recipe_path, Some(&password));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_strip_pspec_xml_suffix_to_get_the_recipe_directory() {
        assert_eq!(
            recipe_directory_path("game/emulator/atari800/pspec.xml"),
            "game/emulator/atari800"
        );
    }

    #[test]
    fn should_compute_a_deterministic_sibling_output_path_not_in_system_temp() {
        let recipes_dir = Path::new("/home/user/contrib");
        let path = build_output_path(recipes_dir, "editor/obsidian/pspec.xml");
        assert_eq!(
            path,
            PathBuf::from("/home/user/pisi-bump-tui-builds/editor/obsidian"),
            "output must live next to recipes_dir, not inside it and not under the system temp dir"
        );
    }

    #[test]
    fn should_compute_the_same_output_path_on_repeated_calls_for_the_same_package() {
        let recipes_dir = Path::new("/home/user/contrib");
        let first = build_output_path(recipes_dir, "editor/obsidian/pspec.xml");
        let second = build_output_path(recipes_dir, "editor/obsidian/pspec.xml");
        assert_eq!(
            first, second,
            "no PID or other per-call uniqueness must be involved — rebuilding the same package must overwrite the same directory"
        );
    }

    #[test]
    fn should_fall_back_to_recipes_dir_itself_when_it_has_no_parent() {
        let recipes_dir = Path::new("/");
        let path = build_output_path(recipes_dir, "editor/obsidian/pspec.xml");
        assert_eq!(
            path,
            PathBuf::from("/pisi-bump-tui-builds/editor/obsidian"),
            "when recipes_dir has no parent (it is the filesystem root), fall back to recipes_dir itself as the base"
        );
    }

    #[test]
    fn should_pass_through_unchanged_when_suffix_is_already_absent() {
        assert_eq!(
            recipe_directory_path("game/emulator/atari800"),
            "game/emulator/atari800"
        );
    }

    #[test]
    fn should_strip_the_real_atari800_recipe_path_from_parity_contrib() {
        let real_pspec_path = std::path::Path::new(
            "/tmp/claude-1000/parity-contrib/game/emulator/atari800/pspec.xml",
        );
        if !real_pspec_path.exists() {
            return;
        }
        let stripped = recipe_directory_path("game/emulator/atari800/pspec.xml");
        let real_directory = std::path::Path::new("/tmp/claude-1000/parity-contrib").join(stripped);
        assert!(
            real_directory.join("pspec.xml").exists(),
            "the stripped directory-only path must match a real pspec.xml location on disk"
        );
    }

    #[test]
    fn should_pass_a_directory_only_script_argument_to_docker_run_even_when_recipe_path_has_pspec_suffix()
     {
        let args = docker_run_args(
            Path::new("/contrib"),
            "game/emulator/atari800/pspec.xml",
            Path::new("/ci"),
            Path::new("/out"),
        );
        let script_argument_index = args
            .iter()
            .position(|arg| arg == "/ci/pisi-build-in-container.sh")
            .expect("script path must be present in the constructed args")
            + 1;
        let script_argument = &args[script_argument_index];
        assert_eq!(
            script_argument, "game/emulator/atari800",
            "the script's recipe_path argument must be directory-only, matching what build_package() does: /git/$1/pspec.xml"
        );
        assert!(
            !script_argument.ends_with("/pspec.xml"),
            "must never pass a path that already ends in /pspec.xml, since the script appends that suffix itself"
        );
    }

    #[test]
    fn should_classify_success_as_available() {
        assert_eq!(
            classify_docker_output(true, ""),
            DockerAvailability::Available
        );
    }

    #[test]
    fn should_classify_permission_denied_stderr_as_permission_denied() {
        let stderr = "Server:\npermission denied while trying to connect to the docker API at unix:///var/run/docker.sock";
        assert_eq!(
            classify_docker_output(false, stderr),
            DockerAvailability::PermissionDenied
        );
    }

    #[test]
    fn should_classify_permission_denied_case_insensitively() {
        let stderr = "PERMISSION DENIED while trying to connect";
        assert_eq!(
            classify_docker_output(false, stderr),
            DockerAvailability::PermissionDenied
        );
    }

    #[test]
    fn should_classify_other_failures_as_daemon_unreachable() {
        let stderr = "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?";
        assert_eq!(
            classify_docker_output(false, stderr),
            DockerAvailability::DaemonUnreachable
        );
    }

    #[test]
    fn should_report_no_message_when_available() {
        assert_eq!(docker_status_message(DockerAvailability::Available), None);
    }

    #[test]
    fn should_report_not_installed_message() {
        assert_eq!(
            docker_status_message(DockerAvailability::NotInstalled),
            Some(DOCKER_NOT_INSTALLED_MESSAGE.to_string())
        );
    }

    #[test]
    fn should_report_daemon_unreachable_message() {
        assert_eq!(
            docker_status_message(DockerAvailability::DaemonUnreachable),
            Some(DOCKER_DAEMON_UNREACHABLE_MESSAGE.to_string())
        );
    }

    #[test]
    fn should_report_permission_denied_message_with_fix_command() {
        let message = docker_status_message(DockerAvailability::PermissionDenied).unwrap();
        assert!(message.contains(DOCKER_PERMISSION_DENIED_MESSAGE));
        assert!(message.contains(DOCKER_PERMISSION_FIX_COMMAND));
    }

    #[test]
    fn should_detect_wrong_password_from_common_sudo_stderr_phrases() {
        assert!(is_wrong_password("Sorry, try again."));
        assert!(is_wrong_password("sudo: 1 incorrect password attempt"));
        assert!(!is_wrong_password(
            "permission denied while trying to connect"
        ));
    }

    #[test]
    fn should_proceed_when_docker_is_available() {
        assert_eq!(
            decide_build_check(DockerAvailability::Available, true),
            BuildCheckOutcome::Proceed
        );
        assert_eq!(
            decide_build_check(DockerAvailability::Available, false),
            BuildCheckOutcome::Proceed
        );
    }

    #[test]
    fn should_offer_sudo_retry_when_permission_denied_and_sudo_is_on_path() {
        let outcome = decide_build_check(DockerAvailability::PermissionDenied, true);
        match outcome {
            BuildCheckOutcome::OfferSudoRetry(message) => {
                assert!(message.contains(DOCKER_PERMISSION_FIX_COMMAND));
            }
            other => panic!("expected OfferSudoRetry, got {other:?}"),
        }
    }

    #[test]
    fn should_block_with_sudo_missing_message_when_permission_denied_and_no_sudo_on_path() {
        assert_eq!(
            decide_build_check(DockerAvailability::PermissionDenied, false),
            BuildCheckOutcome::Blocked(SUDO_MISSING_MESSAGE.to_string())
        );
    }

    #[test]
    fn should_block_with_not_installed_message_regardless_of_sudo_presence() {
        assert_eq!(
            decide_build_check(DockerAvailability::NotInstalled, true),
            BuildCheckOutcome::Blocked(DOCKER_NOT_INSTALLED_MESSAGE.to_string())
        );
        assert_eq!(
            decide_build_check(DockerAvailability::NotInstalled, false),
            BuildCheckOutcome::Blocked(DOCKER_NOT_INSTALLED_MESSAGE.to_string())
        );
    }

    #[test]
    fn should_block_with_daemon_unreachable_message_regardless_of_sudo_presence() {
        assert_eq!(
            decide_build_check(DockerAvailability::DaemonUnreachable, true),
            BuildCheckOutcome::Blocked(DOCKER_DAEMON_UNREACHABLE_MESSAGE.to_string())
        );
    }

    #[test]
    fn should_embed_the_real_build_script_at_compile_time() {
        assert!(BUILD_SCRIPT.contains("build_package"));
        assert!(BUILD_SCRIPT.contains("pisi bi -dy"));
    }

    #[test]
    fn should_collapse_carriage_return_progress_redraws_to_their_final_frame() {
        let raw = "indiriliyor %10\rindiriliyor %49\rindiriliyor %50\rindiriliyor %52\nPaket derleniyor: foo\n";

        let sanitized = sanitize_carriage_return_progress(raw);

        let lines: Vec<&str> = sanitized.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "a multi-frame \\r progress blob followed by one real line must collapse to exactly 2 real lines, not hundreds"
        );
        assert_eq!(
            lines[0], "indiriliyor %52",
            "only the LAST progress frame before the real newline must survive"
        );
        assert_eq!(
            lines[1], "Paket derleniyor: foo",
            "the genuine subsequent line must still be present, verbatim"
        );

        let max_scroll = lines.len().saturating_sub(1);
        assert_eq!(
            lines[max_scroll], "Paket derleniyor: foo",
            "the scroll-to-end position computed on the sanitized text must land on the real line, not a progress fragment"
        );
    }

    #[test]
    fn should_leave_text_without_carriage_returns_unchanged() {
        let raw = "== faz 1 ==\nnormal satır\n== faz 2 ==\n";
        assert_eq!(sanitize_carriage_return_progress(raw), raw);
    }

    #[test]
    fn should_handle_a_trailing_unterminated_progress_line_with_no_final_newline() {
        let raw = "indiriliyor %10\rindiriliyor %99";
        assert_eq!(sanitize_carriage_return_progress(raw), "indiriliyor %99");
    }
}
