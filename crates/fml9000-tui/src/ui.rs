use crate::app::{App, NavItem, Panel, UiMode};
use fml9000_core::MediaItem;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Gauge, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
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

    // Render popup dialogs on top
    match app.ui_mode {
        UiMode::ContextMenu => render_context_menu(f, app),
        UiMode::PlaylistSelect => render_playlist_select(f, app),
        UiMode::NewPlaylistInput => render_new_playlist_input(f, app),
        UiMode::PlaylistContextMenu => render_playlist_menu(f, app),
        UiMode::RenamePlaylistInput => render_rename_playlist_input(f, app),
        UiMode::AddYouTubeChannel => render_add_youtube_input(f, app),
        UiMode::Help => render_help(f),
        UiMode::Normal => {}
    }

    debug!("ui() complete in {:?}", start.elapsed());
}

fn render_now_playing(f: &mut Frame, app: &App, area: Rect) {
    let is_youtube_playing = app.mpv_process.is_some();
    let status = if is_youtube_playing {
        if app.mpv_paused { "⏸" } else { "▶" }
    } else if app.audio.is_playing() {
        "▶"
    } else if app.audio.is_paused() {
        "⏸"
    } else {
        "⏹"
    };

    let (title, artist, album) = if let Some(ref npv) = app.now_playing_video {
        (&npv.video.title as &str, "YouTube", "")
    } else if let Some(ref np) = app.now_playing {
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
    let (ratio, label) = if app.mpv_process.is_some() {
        // YouTube playback - we can show duration if known
        if let Some(ref npv) = app.now_playing_video {
            let elapsed = app.get_youtube_elapsed_secs();
            let pause_indicator = if app.mpv_paused { " [Paused]" } else { "" };
            if let Some(dur) = npv.video.duration_seconds {
                let dur_secs = dur as u64;
                let ratio = if dur_secs > 0 {
                    (elapsed as f64 / dur_secs as f64).min(1.0)
                } else {
                    0.0
                };
                let label = format!(
                    "{}:{:02} / {}:{:02} (YouTube){}",
                    elapsed / 60, elapsed % 60,
                    dur_secs / 60, dur_secs % 60,
                    pause_indicator
                );
                (ratio, label)
            } else {
                (0.0, format!("{}:{:02} (YouTube){}", elapsed / 60, elapsed % 60, pause_indicator))
            }
        } else {
            (0.0, "Playing... (YouTube)".to_string())
        }
    } else if let Some((pos, duration)) = app.get_playback_position() {
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
            Constraint::Length(24),  // Navigation panel
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

    let nav_items = app.build_nav_items();
    let items: Vec<Row> = nav_items.iter().enumerate().map(|(i, item)| {
        let (display_name, is_header) = match item {
            NavItem::SectionHeader(section_id, name) => {
                let expanded = match section_id {
                    crate::app::SectionId::AutoPlaylists => app.section_expanded[0],
                    crate::app::SectionId::UserPlaylists => app.section_expanded[1],
                    crate::app::SectionId::YouTubeChannels => app.section_expanded[2],
                };
                let arrow = if expanded { "▼" } else { "▶" };
                (format!("{} {}", arrow, name), true)
            }
            NavItem::AutoPlaylist(idx, name) => {
                let icon = match idx {
                    0 => "♫",
                    1 => "⋮",
                    2 => "⏮",
                    3 => "✚",
                    _ => " ",
                };
                (format!("  {} {}", icon, name), false)
            }
            NavItem::UserPlaylist(_, name) => {
                (format!("  ♫ {}", name), false)
            }
            NavItem::YouTubeChannel(_, name) => {
                (format!("  📺 {}", name), false)
            }
        };

        let style = if app.nav_state.selected() == Some(i) && is_active {
            Style::default().fg(Color::Black).bg(HIGHLIGHT_COLOR)
        } else if app.nav_state.selected() == Some(i) {
            Style::default().fg(HIGHLIGHT_COLOR)
        } else if is_header {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        Row::new(vec![Cell::from(display_name)]).style(style)
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

    // Calculate max widths for auto-sizing columns (from visible items only)
    let mut max_artist: u16 = 6; // minimum width for "Artist" header
    let mut max_album: u16 = 5;  // minimum width for "Album" header
    let mut max_title: u16 = 5;  // minimum width for "Title" header
    for item in &visible_items {
        max_artist = max_artist.max(item.artist().chars().count() as u16);
        max_album = max_album.max(item.album().chars().count() as u16);
        max_title = max_title.max(item.title().chars().count() as u16);
    }

    // Cap maximums to reasonable values
    max_artist = max_artist.min(30);
    max_album = max_album.min(30);

    // Fixed widths for other columns
    let indicator_width: u16 = 2;
    let duration_width: u16 = 6;
    let date_width: u16 = 12;
    let borders_padding: u16 = 4; // borders and spacing

    // Determine which date columns to show
    let show_both_dates = area.width >= 140;
    let show_last_played = area.width >= 120;

    // Calculate fixed space used
    let fixed_space = indicator_width + duration_width + borders_padding
        + if show_both_dates { date_width * 2 } else if show_last_played { date_width } else { 0 };

    // Available space for artist, album, title
    let available = area.width.saturating_sub(fixed_space);

    // If album is empty/minimal, give more space to title
    let (artist_width, album_width, title_width) = if max_album <= 1 {
        // No album content - split between artist and title
        let artist_w = max_artist.min(available / 3);
        let title_w = available.saturating_sub(artist_w).saturating_sub(2); // 2 for empty album col
        (artist_w, 1, title_w)
    } else {
        // All three columns have content
        let total_content = max_artist + max_album + max_title;
        if total_content <= available {
            // Everything fits - use content widths, give extra to title
            let extra = available.saturating_sub(total_content);
            (max_artist, max_album, max_title + extra)
        } else {
            // Need to compress - distribute proportionally, favor title
            let artist_w = (max_artist as u32 * available as u32 / total_content as u32) as u16;
            let album_w = (max_album as u32 * available as u32 / total_content as u32) as u16;
            let title_w = available.saturating_sub(artist_w).saturating_sub(album_w);
            (artist_w.max(6), album_w.max(1), title_w.max(10))
        }
    };

    let rows: Vec<Row> = visible_items.iter().enumerate().map(|(i, item)| {
        let actual_idx = scroll_offset + i;

        let is_playing = if let Some(ref npv) = app.now_playing_video {
            if let MediaItem::Video(video) = item {
                video.video_id == npv.video.video_id
            } else {
                false
            }
        } else if let Some(ref np) = app.now_playing {
            if let MediaItem::Track(track) = item {
                track.filename == np.track.filename
            } else {
                false
            }
        } else {
            false
        };

        let playing_indicator = if is_playing {
            if app.mpv_paused || app.audio.is_paused() { "⏸ " } else { "▶ " }
        } else {
            "  "
        };

        let style = if selected == actual_idx && is_active {
            Style::default().fg(Color::Black).bg(HIGHLIGHT_COLOR)
        } else if selected == actual_idx {
            Style::default().fg(HIGHLIGHT_COLOR)
        } else if is_playing {
            Style::default().fg(ACCENT_COLOR)
        } else {
            Style::default()
        };

        // Build cells based on available width
        let mut cells = vec![
            Cell::from(playing_indicator),
            Cell::from(item.artist().to_string()),
            Cell::from(item.album().to_string()),
            Cell::from(item.title().to_string()),
            Cell::from(item.duration_str()),
        ];
        if show_both_dates {
            cells.push(Cell::from(item.last_played_str()));
            cells.push(Cell::from(item.added_str()));
        } else if show_last_played {
            cells.push(Cell::from(item.last_played_str()));
        }
        Row::new(cells).style(style)
    }).collect();

    // Build column widths
    let mut widths = vec![
        Constraint::Length(indicator_width),
        Constraint::Length(artist_width),
        Constraint::Length(album_width),
        Constraint::Length(title_width),
        Constraint::Length(duration_width),
    ];
    if show_both_dates {
        widths.push(Constraint::Length(date_width));
        widths.push(Constraint::Length(date_width));
    } else if show_last_played {
        widths.push(Constraint::Length(date_width));
    }

    // Build header based on available width
    let mut header_cells = vec![
        Cell::from(""),
        Cell::from("Artist"),
        Cell::from("Album"),
        Cell::from("Title"),
        Cell::from("Time"),
    ];
    if show_both_dates {
        header_cells.push(Cell::from("Last Played"));
        header_cells.push(Cell::from("Added"));
    } else if show_last_played {
        header_cells.push(Cell::from("Last Played"));
    }
    let header = Row::new(header_cells)
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

    // Render scrollbar if there are more items than visible
    if total_items > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");

        let mut scrollbar_state = ScrollbarState::new(total_items)
            .position(scroll_offset);

        // Render scrollbar in the inner area (inside the border)
        let scrollbar_area = Rect {
            x: area.x + area.width - 2,
            y: area.y + 3, // After border and header
            width: 1,
            height: area.height.saturating_sub(4),
        };
        f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }

    debug!("render_track_list complete in {:?}", render_start.elapsed());
}

fn render_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_text = if app.is_searching {
        "[Enter] Play  [Esc] Cancel search  [Backspace] Delete char"
    } else {
        "[?] Help  [/] Search  [Space] Play/Pause  [n/p] Next/Prev  [a] Queue  [y] Add YT  [s] Shuffle  [r] Repeat  [q] Quit"
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

/// Helper to create a centered popup area
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn render_context_menu(f: &mut Frame, app: &App) {
    let menu = match &app.context_menu {
        Some(m) => m,
        None => return,
    };

    let width = 25;
    let height = (menu.items.len() + 2) as u16; // +2 for borders
    let area = centered_rect(width, height, f.area());

    // Clear the area behind the popup
    f.render_widget(Clear, area);

    let items: Vec<ListItem> = menu.items.iter().enumerate().map(|(i, &item)| {
        let style = if i == menu.selected {
            Style::default().fg(Color::Black).bg(HIGHLIGHT_COLOR)
        } else {
            Style::default()
        };
        ListItem::new(format!(" {} ", item)).style(style)
    }).collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT_COLOR))
                .title(" Action ")
        );

    f.render_widget(list, area);
}

fn render_playlist_select(f: &mut Frame, app: &App) {
    let width = 35;
    let height = (app.playlists.len().min(10) + 2) as u16;
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);

    if app.playlists.is_empty() {
        let msg = Paragraph::new(" No playlists found. Press Esc. ")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT_COLOR))
                    .title(" Select Playlist ")
            );
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app.playlists.iter().enumerate().map(|(i, playlist)| {
        let style = if i == app.playlist_select_idx {
            Style::default().fg(Color::Black).bg(HIGHLIGHT_COLOR)
        } else {
            Style::default()
        };
        ListItem::new(format!(" {} ", playlist.name)).style(style)
    }).collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT_COLOR))
                .title(" Select Playlist ")
        );

    f.render_widget(list, area);
}

fn render_new_playlist_input(f: &mut Frame, app: &App) {
    let width = 40;
    let height = 3;
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);

    let input_text = format!(" {}█", app.new_playlist_name);
    let paragraph = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT_COLOR))
                .title(" New Playlist Name ")
        );

    f.render_widget(paragraph, area);
}

fn render_playlist_menu(f: &mut Frame, app: &App) {
    let menu = match &app.playlist_menu {
        Some(m) => m,
        None => return,
    };

    let width = 25;
    let height = (menu.items.len() + 2) as u16;
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);

    let items: Vec<ListItem> = menu.items.iter().enumerate().map(|(i, &item)| {
        let style = if i == menu.selected {
            Style::default().fg(Color::Black).bg(HIGHLIGHT_COLOR)
        } else {
            Style::default()
        };
        ListItem::new(format!(" {} ", item)).style(style)
    }).collect();

    let title = format!(" {} ", menu.playlist_name);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT_COLOR))
                .title(title)
        );

    f.render_widget(list, area);
}

fn render_rename_playlist_input(f: &mut Frame, app: &App) {
    let width = 40;
    let height = 3;
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);

    let input_text = format!(" {}█", app.rename_playlist_name);
    let paragraph = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT_COLOR))
                .title(" Rename Playlist ")
        );

    f.render_widget(paragraph, area);
}

fn render_add_youtube_input(f: &mut Frame, app: &App) {
    let width = 50;
    let height = 3;
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);

    let input_text = format!(" {}█", app.youtube_channel_url);
    let paragraph = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT_COLOR))
                .title(" Add YouTube Channel (URL or @handle) ")
        );

    f.render_widget(paragraph, area);
}

fn render_help(f: &mut Frame) {
    let help_text = vec![
        "",
        "  Navigation",
        "  ──────────────────────────────",
        "  ↑/↓ or j/k    Move selection",
        "  Tab           Switch panel",
        "  Enter         Play / Select",
        "  /             Search",
        "  Esc           Cancel / Close",
        "  1-5           Quick nav sections",
        "",
        "  Playback",
        "  ──────────────────────────────",
        "  Space         Play / Pause",
        "  n             Next track",
        "  p             Previous track",
        "  s             Stop (or shuffle if stopped)",
        "  S             Toggle shuffle",
        "  r             Cycle repeat mode",
        "",
        "  Actions",
        "  ──────────────────────────────",
        "  a             Add to queue",
        "  y             Add YouTube channel",
        "  Right-click   Context menu",
        "  Double-click  Play track",
        "",
        "  YouTube playback requires mpv",
        "  and yt-dlp to be installed.",
        "",
        "  ?             Show this help",
        "  q             Quit",
        "",
        "  Press any key to close...",
    ];

    let width = 42;
    let height = (help_text.len() + 2) as u16;
    let area = centered_rect(width, height, f.area());

    f.render_widget(Clear, area);

    let text = help_text.join("\n");
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT_COLOR))
                .title(" Help ")
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}
