use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, TableState, Wrap,
};

use crate::app::{App, BuildState, PrepareState, RowState, Screen};
use crate::auth_setup::{AuthOption, AuthSetupState};
use crate::table::{StatusCounts, count_statuses};

const OUTDATED_COLOR: Color = Color::Yellow;
const CURRENT_COLOR: Color = Color::Green;
const UNSUPPORTED_COLOR: Color = Color::DarkGray;
const ERROR_COLOR: Color = Color::Red;
const SCANNING_COLOR: Color = Color::Gray;
const WRITTEN_COLOR: Color = Color::Cyan;
const CHECKING_COLOR: Color = Color::Magenta;

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Table => draw_table_screen(frame, app),
        Screen::Detail | Screen::Confirm => draw_detail_screen(frame, app),
    }
    if app.help_visible {
        draw_help_overlay(frame);
    }
    if app.screen == Screen::Confirm {
        draw_confirm_overlay(frame, app);
    }
}

pub fn draw_auth_setup(frame: &mut Frame, state: &AuthSetupState) {
    let area = centered_rect(70, 50, frame.area());
    frame.render_widget(Clear, area);

    let masked_token = mask_input(&state.token_input);
    let mut lines = vec![
        Line::from(Span::styled(
            "Kimlik doğrulama yöntemi seçin",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        option_line(
            "[1]",
            "gh CLI'nin mevcut oturumunu kullan",
            state.option == Some(AuthOption::GhAuth),
        ),
        option_line(
            "[2]",
            "Token'ı elle gir",
            state.option == Some(AuthOption::Manual),
        ),
        Line::from(""),
    ];

    if state.option == Some(AuthOption::Manual) {
        lines.push(Line::from(format!("Token: {masked_token}_")));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Not: token yerel bir dosyada düz metin olarak saklanacak",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from("Onaylamak için Enter'a basın."));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Kurulum")
                .style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn mask_input(text: &str) -> String {
    text.chars().map(|_| '•').collect()
}

fn option_line(marker: &str, label: &str, selected: bool) -> Line<'static> {
    let text = format!("{marker} {label}");
    if selected {
        Line::from(Span::styled(
            text,
            Style::default().add_modifier(Modifier::REVERSED),
        ))
    } else {
        Line::from(text)
    }
}

fn draw_table_screen(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let gauge_height = if app.scanning { 3 } else { 0 };
    let chunks = Layout::new(
        Direction::Vertical,
        [
            Constraint::Length(1),
            Constraint::Length(gauge_height),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ],
    )
    .split(area);

    frame.render_widget(auth_badge(app), chunks[0]);
    if app.scanning {
        frame.render_widget(scan_gauge(app), chunks[1]);
    }
    let (table, mut table_state) = package_table(app);
    frame.render_stateful_widget(table, chunks[2], &mut table_state);
    frame.render_widget(bottom_line(app), chunks[3]);
    frame.render_widget(table_key_hints(app), chunks[4]);
}

fn auth_badge(app: &App) -> Paragraph<'static> {
    let mut spans = if app.authenticated {
        vec![Span::styled(
            "🔑 Kimlik doğrulandı",
            Style::default().fg(Color::Green),
        )]
    } else {
        vec![Span::styled(
            "⚠ Anonim (60/sa)",
            Style::default().fg(Color::Yellow),
        )]
    };
    if let Some(remaining) = app.rate_limit_remaining {
        spans.push(Span::raw(format!("  ·  kalan istek: {remaining}")));
    }
    Paragraph::new(Line::from(spans))
}

fn scan_gauge(app: &App) -> Gauge<'static> {
    let ratio = if app.scan_total == 0 {
        0.0
    } else {
        app.scan_done as f64 / app.scan_total as f64
    };
    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Taranıyor"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(format!(
            "{}/{} kontrol edildi",
            app.scan_done, app.scan_total
        ))
}

fn status_color(status_text: &str) -> Color {
    match status_text {
        "kontrol ediliyor" => CHECKING_COLOR,
        "yazıldı" => WRITTEN_COLOR,
        "eski" => OUTDATED_COLOR,
        "guncel" => CURRENT_COLOR,
        "hata" => ERROR_COLOR,
        "desteklenmiyor" | "karsilastirilamadi" => UNSUPPORTED_COLOR,
        _ => SCANNING_COLOR,
    }
}

fn package_table(app: &App) -> (Table<'static>, TableState) {
    let visible = app.visible_indices();
    let rows: Vec<Row<'static>> = visible
        .iter()
        .map(|&absolute| {
            let row = &app.rows[absolute];
            let status_text = row.status_text();
            let current = row
                .report
                .as_ref()
                .map(|report| report.current_version.clone())
                .unwrap_or_default();
            let latest = row
                .report
                .as_ref()
                .and_then(|report| report.latest_version.clone())
                .unwrap_or_else(|| "-".to_string());
            let name = if row.written {
                format!("✓ {}", row.name)
            } else {
                row.name.clone()
            };
            let status_cell = Cell::from(status_text.to_string())
                .style(Style::default().fg(status_color(status_text)));
            let cells = vec![
                Cell::from(name),
                Cell::from(current),
                Cell::from(latest),
                status_cell,
            ];
            Row::new(cells)
        })
        .collect();

    let header = Row::new(vec!["Paket", "Mevcut", "Yeni", "Durum"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let title = if app.show_all_statuses {
        "Tüm paketler"
    } else {
        "Eski paketler"
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    if !visible.is_empty() {
        state.select(Some(app.selected.min(visible.len() - 1)));
    }
    (table, state)
}

fn bottom_line(app: &App) -> Paragraph<'static> {
    if let Some(message) = &app.status_message {
        return Paragraph::new(Line::from(Span::styled(
            format!("✓ {message}"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
    }
    let counts = count_statuses(&app.rows);
    Paragraph::new(counts_line(counts))
}

fn counts_line(counts: StatusCounts) -> Line<'static> {
    Line::from(vec![
        Span::raw(format!("Toplam paket: {}  ", counts.total)),
        Span::styled(
            format!("🟢 {} Güncel  ", counts.current),
            Style::default().fg(CURRENT_COLOR),
        ),
        Span::styled(
            format!("🟡 {} Eski  ", counts.outdated),
            Style::default().fg(OUTDATED_COLOR),
        ),
        Span::styled(
            format!("⚫ {} Desteklenmiyor  ", counts.unsupported),
            Style::default().fg(UNSUPPORTED_COLOR),
        ),
        Span::styled(
            format!("⚪ {} Karşılaştırılamadı  ", counts.uncomparable),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            format!("🔴 {} Hata", counts.error),
            Style::default().fg(ERROR_COLOR),
        ),
    ])
}

fn table_key_hints(app: &App) -> Paragraph<'static> {
    let text = if app.filter_mode {
        format!("Ara: {}_   [Esc] Temizle  [Enter] Onayla", app.filter_text)
    } else {
        let toggle_label = if app.show_all_statuses {
            "[o] Sadece Eskileri Göster"
        } else {
            "[o] Tümünü Göster"
        };
        format!(
            "[↑↓/jk] Gezin  {toggle_label}  [/] Ara  [Enter] Detay  [v] Sürüm Kontrol  [r] Yeniden Tara  [A] Giriş Değiştir  [?] Yardım  [q] Çıkış"
        )
    };
    Paragraph::new(text).style(Style::default().fg(Color::Gray))
}

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner_frame(tick: u64) -> &'static str {
    SPINNER_FRAMES[(tick as usize) % SPINNER_FRAMES.len()]
}

fn draw_detail_screen(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let Some(index) = app.detail_index else {
        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(1), Constraint::Min(1)],
        )
        .split(area);
        frame.render_widget(auth_badge(app), chunks[0]);
        frame.render_widget(Paragraph::new("Seçili paket yok."), chunks[1]);
        return;
    };
    let row = &app.rows[index];
    let banner = build_result_banner(&row.build);
    let banner_height = if banner.is_some() { 1 } else { 0 };

    let chunks = Layout::new(
        Direction::Vertical,
        [
            Constraint::Length(1),
            Constraint::Length(8),
            Constraint::Length(banner_height),
            Constraint::Min(3),
            Constraint::Length(1),
        ],
    )
    .split(area);

    frame.render_widget(auth_badge(app), chunks[0]);
    frame.render_widget(detail_info(row, app.spinner_tick), chunks[1]);
    if let Some(banner) = banner {
        frame.render_widget(banner, chunks[2]);
    }
    frame.render_widget(
        detail_pane(row, app.detail_scroll, app.spinner_tick),
        chunks[3],
    );
    frame.render_widget(detail_key_hints(row), chunks[4]);
}

fn build_result_banner(build: &BuildState) -> Option<Paragraph<'static>> {
    let BuildState::Done { success, .. } = build else {
        return None;
    };
    let (text, color) = if *success {
        ("✅  DERLEME BAŞARILI  ✅", Color::Green)
    } else {
        ("❌  DERLEME BAŞARISIZ  ❌", Color::Red)
    };
    Some(
        Paragraph::new(Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::Black)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
    )
}

fn build_status_line(build: &BuildState, tick: u64) -> Option<Line<'static>> {
    let frame = spinner_frame(tick);
    match build {
        BuildState::Idle => None,
        BuildState::CheckingDocker => Some(Line::from(Span::styled(
            format!("Derleme: {frame} Docker kontrol ediliyor..."),
            Style::default().fg(Color::Cyan),
        ))),
        BuildState::Unavailable(reason) => Some(Line::from(Span::styled(
            format!("Derleme: {reason}"),
            Style::default().fg(Color::Red),
        ))),
        BuildState::Pulling => Some(Line::from(Span::styled(
            format!("Derleme: {frame} Docker imajı çekiliyor..."),
            Style::default().fg(Color::Cyan),
        ))),
        BuildState::Building => Some(Line::from(Span::styled(
            format!("Derleme: {frame} çalışıyor..."),
            Style::default().fg(Color::Cyan),
        ))),
        BuildState::Done { .. } => None,
        BuildState::PermissionDenied(_) => Some(Line::from(Span::styled(
            "Derleme: Docker izni yok",
            Style::default().fg(Color::Red),
        ))),
        BuildState::SudoPassword { .. } => Some(Line::from(Span::styled(
            "Derleme: sudo şifresi isteniyor",
            Style::default().fg(Color::Yellow),
        ))),
        BuildState::SudoAuthenticating { .. } => Some(Line::from(Span::styled(
            format!("Derleme: {frame} sudo ile doğrulanıyor..."),
            Style::default().fg(Color::Cyan),
        ))),
    }
}

fn detail_info(row: &RowState, tick: u64) -> Paragraph<'static> {
    let report = row.report.clone();
    let current_version = report
        .as_ref()
        .map(|report| report.current_version.clone())
        .unwrap_or_default();
    let latest_version = report
        .as_ref()
        .and_then(|report| report.latest_version.clone())
        .unwrap_or_else(|| "-".to_string());
    let candidate_url = report
        .as_ref()
        .and_then(|report| report.candidate_url.clone())
        .unwrap_or_else(|| "-".to_string());
    let release_url = report
        .as_ref()
        .and_then(|report| report.release_url.clone())
        .unwrap_or_else(|| "-".to_string());

    let prepare_line = match &row.prepare {
        PrepareState::Idle => Line::from("Hazırlık: beklemede ('p' ile indir ve hazırla)"),
        PrepareState::InProgress => Line::from(Span::styled(
            "Hazırlık: indiriliyor...",
            Style::default().fg(Color::Cyan),
        )),
        PrepareState::Ready { .. } => Line::from(Span::styled(
            "Hazırlık: tamam, diff aşağıda ('w' ile yaz)",
            Style::default().fg(Color::Green),
        )),
        PrepareState::Failed { reason } => Line::from(Span::styled(
            format!("Hazırlık başarısız: {reason}"),
            Style::default().fg(Color::Red),
        )),
    };

    let mut lines = vec![
        Line::from(format!("Tarif: {}", row.recipe_path)),
        Line::from(format!("Mevcut sürüm: {current_version}")),
        Line::from(format!("Yeni sürüm: {latest_version}")),
        Line::from(format!("Aday arşiv: {candidate_url}")),
        Line::from(format!("Upstream yayın: {release_url}")),
        prepare_line,
    ];
    if let Some(build_line) = build_status_line(&row.build, tick) {
        lines.push(build_line);
    }
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(row.name.clone()),
    )
}

fn detail_pane(row: &RowState, scroll: u16, tick: u64) -> Paragraph<'static> {
    let frame = spinner_frame(tick);
    let (title, lines) = match &row.build {
        BuildState::Unavailable(reason) => (
            "Derleme",
            vec![Line::from(Span::styled(
                reason.clone(),
                Style::default().fg(Color::Red),
            ))],
        ),
        BuildState::CheckingDocker => (
            "Derleme",
            vec![Line::from(Span::styled(
                format!("{frame} Docker kontrol ediliyor..."),
                Style::default().fg(Color::Cyan),
            ))],
        ),
        BuildState::Pulling => (
            "Derleme",
            vec![Line::from(Span::styled(
                format!("{frame} Docker imajı çekiliyor..."),
                Style::default().fg(Color::Cyan),
            ))],
        ),
        BuildState::Building => (
            "Derleme",
            vec![Line::from(Span::styled(
                format!("{frame} Derleniyor... (dakikalar sürebilir)"),
                Style::default().fg(Color::Cyan),
            ))],
        ),
        BuildState::Done {
            success,
            log,
            output_dir,
        } => {
            let mut lines = Vec::new();
            if let Some(output_dir) = output_dir {
                lines.push(Line::from(Span::styled(
                    format!("Çıktı: {}", output_dir.display()),
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(""));
            }
            lines.extend(plain_lines(log));
            (
                if *success {
                    "Derleme günlüğü (başarılı ✅)"
                } else {
                    "Derleme günlüğü (başarısız ❌)"
                },
                lines,
            )
        }
        BuildState::PermissionDenied(reason) => (
            "Derleme",
            reason
                .lines()
                .map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Red),
                    ))
                })
                .collect(),
        ),
        BuildState::SudoPassword {
            input,
            error,
            denied_reason,
        } => {
            let masked = mask_input(input);
            let mut lines: Vec<Line<'static>> = denied_reason
                .lines()
                .map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Red),
                    ))
                })
                .collect();
            lines.push(Line::from(""));
            lines.push(Line::from(format!("sudo şifresi: {masked}_")));
            if let Some(error) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    error.clone(),
                    Style::default().fg(Color::Red),
                )));
            }
            ("Sudo ile bir kerelik dene", lines)
        }
        BuildState::SudoAuthenticating { .. } => (
            "Derleme",
            vec![Line::from(Span::styled(
                format!("{frame} sudo ile doğrulanıyor..."),
                Style::default().fg(Color::Cyan),
            ))],
        ),
        BuildState::Idle => match &row.prepare {
            PrepareState::Ready { diff, .. } => ("Diff", colored_diff_lines(diff)),
            PrepareState::Failed { reason } => (
                "Diff",
                vec![Line::from(Span::styled(
                    reason.clone(),
                    Style::default().fg(Color::Red),
                ))],
            ),
            PrepareState::InProgress => (
                "Diff",
                vec![Line::from("İndiriliyor ve sha1 hesaplanıyor...")],
            ),
            PrepareState::Idle => ("Diff", vec![Line::from("Henüz hazırlanmadı.")]),
        },
    };
    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
}

fn plain_lines(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| Line::from(line.to_string()))
        .collect()
}

fn colored_diff_lines(diff: &str) -> Vec<Line<'static>> {
    diff.lines()
        .map(|line| {
            let color = if line.starts_with('+') && !line.starts_with("+++") {
                Some(Color::Green)
            } else if line.starts_with('-') && !line.starts_with("---") {
                Some(Color::Red)
            } else if line.starts_with("@@") {
                Some(Color::Cyan)
            } else {
                None
            };
            match color {
                Some(color) => {
                    Line::from(Span::styled(line.to_string(), Style::default().fg(color)))
                }
                None => Line::from(line.to_string()),
            }
        })
        .collect()
}

fn detail_key_hints(row: &RowState) -> Paragraph<'static> {
    let text = match &row.build {
        BuildState::PermissionDenied(_) => {
            "[s] Sudo İle Bir Kerelik Dene  [Esc] Vazgeç".to_string()
        }
        BuildState::SudoPassword { .. } => "[Enter] Onayla  [Esc] Vazgeç".to_string(),
        BuildState::SudoAuthenticating { .. } => "Doğrulanıyor...".to_string(),
        _ => {
            let prepare_hint = match &row.prepare {
                PrepareState::Ready { .. } => "[p] Yeniden Hazırla  [w] Yaz",
                PrepareState::InProgress => "Hazırlanıyor...",
                _ => "[p] Hazırla",
            };
            format!(
                "{prepare_hint}  [v] Sürüm Kontrol  [b] Docker'da Dene  [↑↓ PgUp PgDn] Kaydır  [Esc] Geri"
            )
        }
    };
    Paragraph::new(text).style(Style::default().fg(Color::Gray))
}

fn draw_confirm_overlay(frame: &mut Frame, app: &App) {
    let Some(index) = app.detail_index else {
        return;
    };
    let row = &app.rows[index];
    let area = centered_rect(60, 20, frame.area());
    frame.render_widget(Clear, area);
    let text = vec![
        Line::from(format!("Bu dosya değiştirilecek: {}", row.recipe_path)),
        Line::from(""),
        Line::from(Span::styled(
            "[y] Onayla  [n] Vazgeç",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Onay")
                .style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_help_overlay(frame: &mut Frame) {
    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(Span::styled(
            "Tablo ekranı",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("[↑↓/jk] Satır Seç"),
        Line::from("[Enter] Eski Paketin Detayına Gir"),
        Line::from("[/] İsme Göre Ara (Esc temizler)"),
        Line::from("[o] Tümünü Göster / Sadece Eskileri Göster"),
        Line::from("[v] Seçili Paketi Tek Başına Kontrol Et"),
        Line::from("[r] Yeniden Tara"),
        Line::from("[A] Kimlik Doğrulama Yöntemini Değiştir"),
        Line::from("[q] Çık"),
        Line::from(""),
        Line::from(Span::styled(
            "Detay ekranı",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("[p] İndir, Sha1 Hesapla, pspec.xml Hazırla"),
        Line::from("[w] Hazırlanan Değişikliği Yazma Onayına Gönder"),
        Line::from("[v] Bu Paketi Tek Başına Kontrol Et"),
        Line::from("[b] Diskteki pspec.xml'i Docker'da Derle"),
        Line::from("[s] Docker İzni Yoksa Sudo İle Bir Kerelik Dene"),
        Line::from("[↑↓ PgUp PgDn] Diff / Derleme Günlüğünü Kaydır"),
        Line::from("[Esc] Tabloya Dön"),
        Line::from(""),
        Line::from(Span::styled(
            "Onay ekranı",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("[y] pspec.xml'i Diske Yaz"),
        Line::from("[n] / [Esc] Yazmadan Vazgeç"),
        Line::from(""),
        Line::from("[?] / [Esc] Bu Yardımı Kapat"),
    ];
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Kısayollar")
            .style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::new(
        Direction::Vertical,
        [
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ],
    )
    .split(area);
    Layout::new(
        Direction::Horizontal,
        [
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ],
    )
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pisi_bump_bot::report_model::{PackageReport, Status};
    use pisi_bump_common::PackageRecipe;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::{draw, draw_auth_setup};
    use crate::app::{App, WorkerMessage};
    use crate::auth_setup::{AuthOption, AuthSetupState};

    fn recipe(name: &str, path: &str) -> PackageRecipe {
        PackageRecipe {
            recipe_path: path.to_string(),
            name: name.to_string(),
            current_version: "1.0".to_string(),
            current_release: 1,
            archive_urls: vec!["https://github.com/o/r/archive/1.0.tar.gz".to_string()],
        }
    }

    fn render(app: &App, width: u16, height: u16) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| draw(frame, app))
            .expect("draw should not fail");
    }

    #[test]
    fn should_render_table_screen_while_scanning_without_panicking() {
        let recipes = vec![
            recipe("brave-browser", "network/browser/brave/pspec.xml"),
            recipe("jedit", "editor/jedit/pspec.xml"),
        ];
        let app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], false);
        render(&app, 100, 30);
    }

    #[test]
    fn should_scroll_table_viewport_to_keep_selection_visible_with_many_rows() {
        let recipes: Vec<PackageRecipe> = (0..140)
            .map(|index| {
                recipe(
                    &format!("pkg-{index:03}"),
                    &format!("pkg-{index:03}/pspec.xml"),
                )
            })
            .collect();
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        for (index, recipe) in recipes.iter().enumerate() {
            app.apply(WorkerMessage::ScanProgress {
                index,
                done: index + 1,
                total: recipes.len(),
                report: Box::new(PackageReport::new(&recipe.name, Status::Current, "1.0")),
            });
        }
        app.apply(WorkerMessage::ScanComplete);
        app.show_all_statuses = true;
        app.selected = app.visible_indices().len() - 1;

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");

        let rendered = buffer_text(&terminal);
        assert!(
            rendered.contains("pkg-139"),
            "viewport must scroll down far enough to show the last selected row"
        );
        assert!(
            !rendered.contains("pkg-000"),
            "row 0 must have scrolled out of view once row 139 is selected"
        );
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let width = buffer.area.width as usize;
        buffer
            .content()
            .chunks(width)
            .map(|line| {
                let mut text: String = line.iter().map(|cell| cell.symbol()).collect();
                text.push('\n');
                text
            })
            .collect()
    }

    #[test]
    fn should_render_table_screen_with_mixed_statuses_and_unreadable_rows() {
        let recipes = vec![
            recipe("brave-browser", "network/browser/brave/pspec.xml"),
            recipe("jedit", "editor/jedit/pspec.xml"),
            recipe("atari800", "games/emulator/atari800/pspec.xml"),
        ];
        let mut app = App::new(
            PathBuf::from("/tmp/contrib"),
            &recipes,
            &["broken/pspec.xml".to_string()],
            true,
        );
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 3,
            report: Box::new(
                PackageReport::new("brave-browser", Status::Outdated, "1.93.129")
                    .with_latest_version("1.94.1")
                    .with_candidate_url(Some("https://example.org/brave-1.94.1.zip".to_string()))
                    .with_release_url("https://github.com/brave/brave-browser/releases/tag/v1.94.1")
                    .with_recipe_path("network/browser/brave/pspec.xml"),
            ),
        });
        app.apply(WorkerMessage::ScanProgress {
            index: 1,
            done: 2,
            total: 3,
            report: Box::new(PackageReport::new("jedit", Status::Current, "5.6.0")),
        });
        app.apply(WorkerMessage::ScanProgress {
            index: 2,
            done: 3,
            total: 3,
            report: Box::new(PackageReport::new("atari800", Status::Error, "4.2.0")),
        });
        app.apply(WorkerMessage::ScanComplete);
        app.apply(WorkerMessage::RateLimitUpdate { remaining: 58 });
        app.show_all_statuses = true;
        render(&app, 100, 30);

        app.filter_mode = true;
        app.filter_text = "brave".to_string();
        render(&app, 100, 30);

        app.filter_mode = false;
        app.status_message =
            Some("pspec.xml güncellendi (network/browser/brave/pspec.xml)".to_string());
        render(&app, 100, 30);

        app.help_visible = true;
        render(&app, 100, 30);
    }

    #[test]
    fn should_render_detail_screen_for_every_prepare_state_without_panicking() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(
                PackageReport::new("brave-browser", Status::Outdated, "1.93.129")
                    .with_latest_version("1.94.1")
                    .with_candidate_url(Some("https://example.org/brave-1.94.1.zip".to_string()))
                    .with_recipe_path("network/browser/brave/pspec.xml"),
            ),
        });
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);
        render(&app, 100, 30);

        app.rows[0].prepare = crate::app::PrepareState::InProgress;
        render(&app, 100, 30);

        app.rows[0].prepare = crate::app::PrepareState::Ready {
            diff: "--- a/pspec.xml\n+++ b/pspec.xml\n@@ -1,2 +1,2 @@\n-old\n+new\n context\n"
                .to_string(),
            new_pspec_text: "<PISI/>".to_string(),
        };
        render(&app, 100, 30);
        app.detail_scroll = 3;
        render(&app, 100, 30);

        app.rows[0].prepare = crate::app::PrepareState::Failed {
            reason: "indirme başarısız".to_string(),
        };
        render(&app, 100, 30);
    }

    #[test]
    fn should_render_detail_screen_for_every_build_state_without_panicking() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.rows[0].written = true;
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);

        for build in [
            crate::app::BuildState::CheckingDocker,
            crate::app::BuildState::Unavailable("Docker bulunamadı veya çalışmıyor".to_string()),
            crate::app::BuildState::Pulling,
            crate::app::BuildState::Building,
            crate::app::BuildState::Done {
                success: true,
                log: "== Paket derleniyor ==\nbasarili\n".to_string(),
                output_dir: Some(PathBuf::from("/tmp/contrib-builds/network/browser/brave")),
            },
            crate::app::BuildState::Done {
                success: false,
                log: "== Paket derleniyor ==\nhata\n".to_string(),
                output_dir: None,
            },
        ] {
            app.rows[0].build = build;
            render(&app, 100, 30);
        }
    }

    #[test]
    fn should_render_the_output_directory_path_in_the_detail_pane_when_build_succeeds() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].build = crate::app::BuildState::Done {
            success: true,
            log: "== tamam ==\n".to_string(),
            output_dir: Some(PathBuf::from("/tmp/contrib-builds/network/browser/brave")),
        };

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");
        let rendered = buffer_text(&terminal);

        assert!(
            rendered.contains("/tmp/contrib-builds/network/browser/brave"),
            "the real output directory path must be visible in the detail pane so the user can find the built .pisi file"
        );
    }

    #[test]
    fn should_not_render_an_output_path_line_when_the_build_never_produced_one() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].build = crate::app::BuildState::Done {
            success: false,
            log: "derleme betiği hazırlanamadı\n".to_string(),
            output_dir: None,
        };

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");
        let rendered = buffer_text(&terminal);

        assert!(
            !rendered.contains("Çıktı:"),
            "no output path line should appear when the build failed before an output directory existed"
        );
    }

    #[test]
    fn should_render_sudo_retry_states_and_never_leak_the_password_text() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.rows[0].written = true;
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);

        app.rows[0].build = crate::app::BuildState::PermissionDenied(
            "Docker soketine erişim izniniz yok.".to_string(),
        );
        render(&app, 100, 30);

        app.rows[0].build = crate::app::BuildState::SudoPassword {
            input: "super-secret-password".to_string(),
            error: None,
            denied_reason: "Docker soketine erişim izniniz yok.".to_string(),
        };
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");
        let rendered = buffer_text(&terminal);
        assert!(
            !rendered.contains("super-secret-password"),
            "the sudo password must never be rendered on screen, only masked dots"
        );

        app.rows[0].build = crate::app::BuildState::SudoPassword {
            input: String::new(),
            error: Some("Yanlış şifre, tekrar deneyin".to_string()),
            denied_reason: "Docker soketine erişim izniniz yok.".to_string(),
        };
        render(&app, 100, 30);

        app.rows[0].build = crate::app::BuildState::SudoAuthenticating {
            denied_reason: "Docker soketine erişim izniniz yok.".to_string(),
        };
        render(&app, 100, 30);
    }

    #[test]
    fn should_render_build_result_banner_as_its_own_element_even_on_small_terminal() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.rows[0].written = true;
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].build = crate::app::BuildState::Done {
            success: true,
            log: "l1\nl2\nl3\n".to_string(),
            output_dir: Some(PathBuf::from("/tmp/contrib-builds/network/browser/brave")),
        };

        for (width, height) in [(100, 30), (40, 10)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("terminal should initialize");
            terminal
                .draw(|frame| draw(frame, &app))
                .expect("draw should not fail");
            let rendered = buffer_text(&terminal);
            assert!(
                rendered.contains("DERLEME BAŞARILI"),
                "the pass/fail banner must stay visible even on a {width}x{height} terminal"
            );
        }

        app.rows[0].build = crate::app::BuildState::Done {
            success: false,
            log: "l1\n".to_string(),
            output_dir: None,
        };
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");
        assert!(buffer_text(&terminal).contains("DERLEME BAŞARISIZ"));
    }

    #[test]
    fn should_not_render_banner_while_build_is_still_in_progress() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.rows[0].written = true;
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].build = crate::app::BuildState::Building;

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");
        let rendered = buffer_text(&terminal);
        assert!(!rendered.contains("DERLEME BAŞARILI"));
        assert!(!rendered.contains("DERLEME BAŞARISIZ"));
    }

    #[test]
    fn should_advance_the_spinner_frame_across_different_tick_values() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.rows[0].written = true;
        app.screen = crate::app::Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].build = crate::app::BuildState::Building;

        app.spinner_tick = 0;
        let backend_a = TestBackend::new(100, 30);
        let mut terminal_a = Terminal::new(backend_a).expect("terminal should initialize");
        terminal_a
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");
        let frame_a = buffer_text(&terminal_a);

        app.spinner_tick = 3;
        let backend_b = TestBackend::new(100, 30);
        let mut terminal_b = Terminal::new(backend_b).expect("terminal should initialize");
        terminal_b
            .draw(|frame| draw(frame, &app))
            .expect("draw should not fail");
        let frame_b = buffer_text(&terminal_b);

        assert_ne!(
            frame_a, frame_b,
            "two different spinner ticks must render visibly different frames, proving the indicator actually animates"
        );
    }

    #[test]
    fn should_render_confirm_overlay_without_panicking() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.rows[0].prepare = crate::app::PrepareState::Ready {
            diff: "diff".to_string(),
            new_pspec_text: "<PISI/>".to_string(),
        };
        app.screen = crate::app::Screen::Confirm;
        app.detail_index = Some(0);
        render(&app, 100, 30);
    }

    #[test]
    fn should_render_on_very_small_terminal_without_panicking() {
        let app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], true);
        render(&app, 20, 6);
    }

    #[test]
    fn should_render_auth_setup_screen_for_every_state_without_panicking() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");

        let mut state = AuthSetupState::new();
        terminal
            .draw(|frame| draw_auth_setup(frame, &state))
            .expect("draw should not fail");

        state.option = Some(AuthOption::GhAuth);
        terminal
            .draw(|frame| draw_auth_setup(frame, &state))
            .expect("draw should not fail");

        state.option = Some(AuthOption::Manual);
        state.token_input = "some-token-text".to_string();
        terminal
            .draw(|frame| draw_auth_setup(frame, &state))
            .expect("draw should not fail");

        let rendered = buffer_text(&terminal);
        assert!(
            !rendered.contains("some-token-text"),
            "the raw token text must never be rendered on screen, only masked dots"
        );
        assert!(rendered.contains("düz metin olarak saklanacak"));
    }
}
