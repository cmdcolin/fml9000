use crate::app::{App, Panel, DisplayItem};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table},
    Frame,
};
use tracing::debug;
use std::time::Instant;

const ACCENT_COLOR: Color = Color::Cyan;
const HIGHLIGHT_COLOR: Color = Color::Yellow;
const DIM_COLOR: Color = Color::DarkGray;

pub fn ui(f: &mut Frame, app: &mut App) {
    let start = Instant::now();
    debug!("ui() start");
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Now playing bar
            Constraint::Length(2),  // Progress bar
            Constraint::Min(10),    // Main content
            Constraint::Length(1),  // Help bar
        ])
        .split(f.area());

    render_now_playing(f, app, chunks[0]);
    render_progress_bar(f, app, chunks[1]);
    render_main_content(f, app, chunks[2]);
    render_help_bar(f, app, chunks[3]);

    debug!("ui() complete in {:?}", start.elapsed());
}

fn render_now_playing(f: &mut Frame, app: &App, area: Rect) {
    let status = if app.audio.is_playing() {
        "▶"
    } else if app.audio.is_paused() {
        "⏸"
    } else {
        "⏹"
    };

    let (title, artist, album) = if let Some(ref np) = app.now_playing {
        (
            np.track.title.as_deref().unwrap_or("Unknown"),
            np.track.artist.as_deref().unwrap_or("Unknown"),
            np.track.album.as_deref().unwrap_or("Unknown"),
        )
    } else {
        ("No track playing", "", "")
    };

    let shuffle_indicator = if app.shuffle_enabled { " 🔀" } else { "" };
    let repeat_indicator = match app.repeat_mode {
        fml9000_core::settings::RepeatMode::Off => "",
        fml9000_core::settings::RepeatMode::All => " 🔁",
        fml9000_core::settings::RepeatMode::One => " 🔂",
    };

    let text = if artist.is_empty() {
        format!("{} {}{}{}", status, title, shuffle_indicator, repeat_indicator)
    } else {
        format!("{} {} - {} - {}{}{}", status, artist, album, title, shuffle_indicator, repeat_indicator)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" fml9000-tui ")
        .title_style(Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD));

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn render_progress_bar(f: &mut Frame, app: &App, area: Rect) {
    let (ratio, label) = if let Some((pos, duration)) = app.get_playback_position() {
        let pos_secs = pos.as_secs();
        let dur_secs = duration.as_secs();
        let ratio = if dur_secs > 0 {
            pos_secs as f64 / dur_secs as f64
        } else {
            0.0
        };
        let label = format!(
            "{}:{:02} / {}:{:02}",
            pos_secs / 60, pos_secs % 60,
            dur_secs / 60, dur_secs % 60
        );
        (ratio, label)
    } else {
        (0.0, "0:00 / 0:00".to_string())
    };

    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(ACCENT_COLOR).bg(Color::DarkGray))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(label);

    f.render_widget(gauge, area);
}

fn render_main_content(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),  // Navigation panel
            Constraint::Min(40),     // Track list
        ])
        .split(area);

    render_navigation(f, app, chunks[0]);
    render_track_list(f, app, chunks[1]);
}

fn render_navigation(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Navigation;
    let border_style = if is_active {
        Style::default().fg(HIGHLIGHT_COLOR)
    } else {
        Style::default().fg(DIM_COLOR)
    };

    let items: Vec<Row> = app.nav_items().iter().enumerate().map(|(i, name)| {
        let prefix = match i {
            0 => "♫",
            1 => "☰",
            2 => "⋮",
            3 => "⏮",
            4 => "✚",
            _ => " ",
        };

        let style = if app.nav_state.selected() == Some(i) && is_active {
            Style::default().fg(Color::Black).bg(HIGHLIGHT_COLOR)
        } else if app.nav_state.selected() == Some(i) {
            Style::default().fg(HIGHLIGHT_COLOR)
        } else {
            Style::default()
        };

        Row::new(vec![
            Cell::from(format!(" {} {}", prefix, name))
        ]).style(style)
    }).collect();

    let table = Table::new(items, [Constraint::Percentage(100)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Navigation ")
        )
        .style(Style::default().fg(Color::White));

    let mut state = app.nav_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_track_list(f: &mut Frame, app: &mut App, area: Rect) {
    let render_start = Instant::now();

    let is_active = app.active_panel == Panel::TrackList;
    let border_style = if is_active {
        Style::default().fg(HIGHLIGHT_COLOR)
    } else {
        Style::default().fg(DIM_COLOR)
    };

    let total_items = app.get_total_items();
    let title = if app.is_searching {
        format!(" Tracks [/{}] ({} matches) ", app.search_query, total_items)
    } else {
        format!(" Tracks ({}) ", total_items)
    };

    // Calculate visible height from area (subtract borders and header)
    // area.height - 2 (borders) - 2 (header + margin)
    let visible_height = area.height.saturating_sub(4) as usize;
    app.set_visible_height(visible_height);
    app.update_scroll_for_selection();

    // Get only visible items (virtual scrolling)
    let (visible_items, scroll_offset) = app.get_visible_items();
    debug!("Rendering {} visible items (offset {}, total {})", visible_items.len(), scroll_offset, total_items);

    let selected = app.track_state.selected().unwrap_or(0);

    let rows: Vec<Row> = visible_items.iter().enumerate().map(|(i, item)| {
        let actual_idx = scroll_offset + i;

        let is_playing = if let Some(ref np) = app.now_playing {
            if let DisplayItem::Track(track) = item {
                track.filename == np.track.filename
            } else {
                false
            }
        } else {
            false
        };

        let playing_indicator = if is_playing { "▶ " } else { "  " };

        let style = if selected == actual_idx && is_active {
            Style::default().fg(Color::Black).bg(HIGHLIGHT_COLOR)
        } else if selected == actual_idx {
            Style::default().fg(HIGHLIGHT_COLOR)
        } else if is_playing {
            Style::default().fg(ACCENT_COLOR)
        } else {
            Style::default()
        };

        Row::new(vec![
            Cell::from(playing_indicator),
            Cell::from(item.artist().to_string()),
            Cell::from(item.album().to_string()),
            Cell::from(item.title().to_string()),
            Cell::from(item.duration_str()),
        ]).style(style)
    }).collect();

    let widths = [
        Constraint::Length(2),      // Playing indicator
        Constraint::Percentage(25), // Artist
        Constraint::Percentage(25), // Album
        Constraint::Percentage(40), // Title
        Constraint::Length(6),      // Duration
    ];

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("Artist"),
        Cell::from("Album"),
        Cell::from("Title"),
        Cell::from("Time"),
    ])
    .style(Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD))
    .bottom_margin(1);

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title)
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(table, area);
    debug!("render_track_list complete in {:?}", render_start.elapsed());
}

fn render_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_text = if app.is_searching {
        "[Enter] Play  [Esc] Cancel search  [Backspace] Delete char"
    } else {
        "[/] Search  [Space] Play/Pause  [n/p] Next/Prev  [a] Add to queue  [s] Shuffle  [r] Repeat  [Tab] Switch panel  [q] Quit"
    };

    let status = if let Some(ref msg) = app.status_message {
        format!(" {} | {}", msg, help_text)
    } else {
        format!(" {}", help_text)
    };

    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(DIM_COLOR));

    f.render_widget(paragraph, area);
}
