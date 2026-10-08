use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, Wrap};

use crate::app::{App, PrepareState, Screen};
use crate::table::{StatusCounts, count_statuses};

const OUTDATED_COLOR: Color = Color::Yellow;
const CURRENT_COLOR: Color = Color::Green;
const UNSUPPORTED_COLOR: Color = Color::DarkGray;
const ERROR_COLOR: Color = Color::Red;
const SCANNING_COLOR: Color = Color::Gray;

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
    frame.render_widget(package_table(app), chunks[2]);
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
        "eski" => OUTDATED_COLOR,
        "guncel" => CURRENT_COLOR,
        "hata" => ERROR_COLOR,
        "desteklenmiyor" | "karsilastirilamadi" => UNSUPPORTED_COLOR,
        _ => SCANNING_COLOR,
    }
}

fn package_table(app: &App) -> Table<'static> {
    let visible = app.visible_indices();
    let rows: Vec<Row<'static>> = visible
        .iter()
        .enumerate()
        .map(|(position, &absolute)| {
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
            let mut cells = vec![
                Cell::from(name),
                Cell::from(current),
                Cell::from(latest),
                status_cell,
            ];
            let mut style = Style::default();
            if position == app.selected {
                style = style.add_modifier(Modifier::REVERSED);
                cells = cells
                    .into_iter()
                    .map(|cell| cell.style(Style::default().add_modifier(Modifier::REVERSED)))
                    .collect();
            }
            Row::new(cells).style(style)
        })
        .collect();

    let header = Row::new(vec!["Paket", "Mevcut", "Yeni", "Durum"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let title = if app.show_all_statuses {
        "Tüm paketler"
    } else {
        "Eski paketler"
    };

    Table::new(
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
        format!("Ara: {}_   (Esc temizle, Enter onayla)", app.filter_text)
    } else {
        "↑↓/jk gezin · Enter detay · / ara · o tümünü göster · r tekrar tara · ? yardım · q çıkış"
            .to_string()
    };
    Paragraph::new(text).style(Style::default().fg(Color::Gray))
}

fn draw_detail_screen(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::new(
        Direction::Vertical,
        [
            Constraint::Length(1),
            Constraint::Length(7),
            Constraint::Min(3),
            Constraint::Length(1),
        ],
    )
    .split(area);

    frame.render_widget(auth_badge(app), chunks[0]);

    let Some(index) = app.detail_index else {
        frame.render_widget(Paragraph::new("Seçili paket yok."), chunks[1]);
        return;
    };
    let row = &app.rows[index];
    frame.render_widget(detail_info(row), chunks[1]);
    frame.render_widget(diff_pane(row, app.detail_scroll), chunks[2]);
    frame.render_widget(detail_key_hints(row), chunks[3]);
}

fn detail_info(row: &crate::app::RowState) -> Paragraph<'static> {
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

    let lines = vec![
        Line::from(format!("Tarif: {}", row.recipe_path)),
        Line::from(format!("Mevcut sürüm: {current_version}")),
        Line::from(format!("Yeni sürüm: {latest_version}")),
        Line::from(format!("Aday arşiv: {candidate_url}")),
        Line::from(format!("Upstream yayın: {release_url}")),
        prepare_line,
    ];
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(row.name.clone()),
    )
}

fn diff_pane(row: &crate::app::RowState, scroll: u16) -> Paragraph<'static> {
    let lines = match &row.prepare {
        PrepareState::Ready { diff, .. } => colored_diff_lines(diff),
        PrepareState::Failed { reason } => {
            vec![Line::from(Span::styled(
                reason.clone(),
                Style::default().fg(Color::Red),
            ))]
        }
        PrepareState::InProgress => vec![Line::from("İndiriliyor ve sha1 hesaplanıyor...")],
        PrepareState::Idle => vec![Line::from("Henüz hazırlanmadı.")],
    };
    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Diff"))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
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

fn detail_key_hints(row: &crate::app::RowState) -> Paragraph<'static> {
    let text = match &row.prepare {
        PrepareState::Ready { .. } => {
            "p yeniden hazırla · w yaz · ↑↓ diff kaydır · Esc tabloya dön"
        }
        PrepareState::InProgress => "hazırlanıyor... · Esc tabloya dön",
        _ => "p hazırla · ↑↓ diff kaydır · Esc tabloya dön",
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
            "[y] Onayla     [n] Vazgeç",
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
        Line::from("↑/k, ↓/j   satır seç"),
        Line::from("Enter      eski paketin detayına gir"),
        Line::from("/          isme göre ara (Esc temizler)"),
        Line::from("o          tüm durumları göster / sadece eski"),
        Line::from("r          yeniden tara"),
        Line::from("q          çık"),
        Line::from(""),
        Line::from(Span::styled(
            "Detay ekranı",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("p          indir, sha1 hesapla, pspec.xml hazırla"),
        Line::from("w          hazırlanan değişikliği yazma onayına gönder"),
        Line::from("↑/↓, PgUp/PgDn   diff'i kaydır"),
        Line::from("Esc        tabloya dön"),
        Line::from(""),
        Line::from(Span::styled(
            "Onay ekranı",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("y          pspec.xml'i diske yaz"),
        Line::from("n / Esc    yazmadan vazgeç"),
        Line::from(""),
        Line::from("? veya Esc ile bu yardımı kapat"),
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

    use super::draw;
    use crate::app::{App, WorkerMessage};

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
}
