mod app;
mod ui;

use app::{App, Panel, UiMode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Size, Terminal};
use std::io;
use std::time::Duration;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> io::Result<()> {
    // Setup file logging
    let file_appender = tracing_appender::rolling::never("/tmp", "fml9000-tui.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_max_level(Level::DEBUG)
        .init();

    info!("Starting fml9000-tui");

    // Initialize database
    info!("Initializing database...");
    if let Err(e) = fml9000_core::init_db() {
        eprintln!("Failed to initialize database: {}", e);
    }
    info!("Database initialized");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    info!("Creating app...");
    let mut app = App::new();
    info!("App created, {} tracks loaded", app.tracks.len());

    // Show audio error if any
    if let Some(ref err) = app.audio_error {
        app.status_message = Some(format!("Audio error: {}", err));
    }

    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(500);

    info!("Entering main loop");

    loop {
        // Draw UI first
        terminal.draw(|f| ui::ui(f, app))?;

        // Calculate timeout until next tick
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        // Wait for an event (with timeout for tick)
        if event::poll(timeout)? {
            // Drain ALL pending events before next draw
            while event::poll(Duration::ZERO)? {
                let ev = event::read()?;
                if handle_event(app, &ev, terminal.size()?)? {
                    return Ok(());
                }
            }
        }

        // Check if it's time for a tick
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = std::time::Instant::now();
        }
    }
}

/// Handle a single event. Returns true if the app should quit.
fn handle_event(app: &mut App, event: &Event, size: Size) -> io::Result<bool> {
    // Handle based on current UI mode
    match app.ui_mode {
        UiMode::ContextMenu => return handle_context_menu_event(app, event, size),
        UiMode::PlaylistSelect => return handle_playlist_select_event(app, event, size),
        UiMode::NewPlaylistInput => return handle_new_playlist_event(app, event, size),
        UiMode::PlaylistContextMenu => return handle_playlist_menu_event(app, event, size),
        UiMode::RenamePlaylistInput => return handle_rename_playlist_event(app, event, size),
        UiMode::Help => return handle_help_event(app, event),
        UiMode::Normal => {}
    }

    match event {
        Event::Mouse(mouse) => {
            // Ignore mouse move events
            if !matches!(mouse.kind, MouseEventKind::Moved) {
                handle_mouse(app, mouse.kind, mouse.column, mouse.row, size);
            }
        }
        Event::Key(key) => {
            // Only handle key press events, not releases
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }

            // Handle search mode separately
            if app.is_searching {
                match key.code {
                    KeyCode::Esc => app.cancel_search(),
                    KeyCode::Enter => {
                        app.play_selected();
                        app.cancel_search();
                    }
                    KeyCode::Backspace => app.search_backspace(),
                    KeyCode::Char(c) => app.update_search(c),
                    KeyCode::Up => app.track_up(),
                    KeyCode::Down => app.track_down(),
                    _ => {}
                }
                return Ok(false);
            }

            // Normal mode key handling
            match key.code {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Char('?') => app.show_help(),
                KeyCode::Char('/') => app.start_search(),
                KeyCode::Tab => app.toggle_panel(),
                KeyCode::Char(' ') => {
                    if app.now_playing.is_some() || app.mpv_process.is_some() {
                        app.toggle_pause();
                    } else {
                        app.play_selected();
                    }
                }
                KeyCode::Enter => {
                    if app.active_panel == Panel::Navigation {
                        app.select_nav();
                    } else {
                        app.play_selected();
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if app.active_panel == Panel::Navigation {
                        app.nav_up();
                    } else {
                        app.track_up();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.active_panel == Panel::Navigation {
                        app.nav_down();
                    } else {
                        app.track_down();
                    }
                }
                KeyCode::Char('n') => app.play_next(),
                KeyCode::Char('p') => app.play_prev(),
                KeyCode::Char('s') => {
                    if app.now_playing.is_some() {
                        app.stop();
                    } else {
                        app.toggle_shuffle();
                    }
                }
                KeyCode::Char('S') => app.toggle_shuffle(),
                KeyCode::Char('r') => app.cycle_repeat(),
                KeyCode::Char('a') => app.queue_selected(),
                KeyCode::Char('1') => {
                    app.nav_state.select(Some(0));
                    app.select_nav();
                }
                KeyCode::Char('2') => {
                    app.nav_state.select(Some(1));
                    app.select_nav();
                }
                KeyCode::Char('3') => {
                    app.nav_state.select(Some(2));
                    app.select_nav();
                }
                KeyCode::Char('4') => {
                    app.nav_state.select(Some(3));
                    app.select_nav();
                }
                KeyCode::Char('5') => {
                    app.nav_state.select(Some(4));
                    app.select_nav();
                }
                _ => {}
            }
        }
        Event::Resize(_, _) => {}
        _ => {}
    }
    Ok(false)
}

fn handle_context_menu_event(app: &mut App, event: &Event, size: Size) -> io::Result<bool> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            match key.code {
                KeyCode::Esc => app.close_context_menu(),
                KeyCode::Up | KeyCode::Char('k') => app.context_menu_up(),
                KeyCode::Down | KeyCode::Char('j') => app.context_menu_down(),
                KeyCode::Enter => app.context_menu_select(),
                _ => {}
            }
        }
        Event::Mouse(mouse) => {
            if let Some(ref menu) = app.context_menu {
                let width: u16 = 25;
                let height = (menu.items.len() + 2) as u16;
                let popup_x = (size.width.saturating_sub(width)) / 2;
                let popup_y = (size.height.saturating_sub(height)) / 2;

                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Check if click is inside popup
                        if mouse.column >= popup_x && mouse.column < popup_x + width
                            && mouse.row >= popup_y && mouse.row < popup_y + height
                        {
                            // Click on menu item (account for border)
                            let item_row = mouse.row.saturating_sub(popup_y + 1);
                            if (item_row as usize) < menu.items.len() {
                                app.context_menu.as_mut().unwrap().selected = item_row as usize;
                                app.context_menu_select();
                            }
                        } else {
                            // Click outside closes menu
                            app.close_context_menu();
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_playlist_select_event(app: &mut App, event: &Event, size: Size) -> io::Result<bool> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            match key.code {
                KeyCode::Esc => app.close_context_menu(),
                KeyCode::Up | KeyCode::Char('k') => app.playlist_select_up(),
                KeyCode::Down | KeyCode::Char('j') => app.playlist_select_down(),
                KeyCode::Enter => app.playlist_select_confirm(),
                _ => {}
            }
        }
        Event::Mouse(mouse) => {
            let width: u16 = 35;
            let height = (app.playlists.len().min(10) + 2) as u16;
            let popup_x = (size.width.saturating_sub(width)) / 2;
            let popup_y = (size.height.saturating_sub(height)) / 2;

            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if mouse.column >= popup_x && mouse.column < popup_x + width
                        && mouse.row >= popup_y && mouse.row < popup_y + height
                    {
                        let item_row = mouse.row.saturating_sub(popup_y + 1);
                        if (item_row as usize) < app.playlists.len() {
                            app.playlist_select_idx = item_row as usize;
                            app.playlist_select_confirm();
                        }
                    } else {
                        app.close_context_menu();
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_new_playlist_event(app: &mut App, event: &Event, size: Size) -> io::Result<bool> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            match key.code {
                KeyCode::Esc => app.close_context_menu(),
                KeyCode::Enter => app.new_playlist_confirm(),
                KeyCode::Backspace => app.new_playlist_backspace(),
                KeyCode::Char(c) => app.new_playlist_input(c),
                _ => {}
            }
        }
        Event::Mouse(mouse) => {
            let width: u16 = 40;
            let height: u16 = 3;
            let popup_x = (size.width.saturating_sub(width)) / 2;
            let popup_y = (size.height.saturating_sub(height)) / 2;

            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Click outside closes dialog
                    if !(mouse.column >= popup_x && mouse.column < popup_x + width
                        && mouse.row >= popup_y && mouse.row < popup_y + height)
                    {
                        app.close_context_menu();
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_playlist_menu_event(app: &mut App, event: &Event, size: Size) -> io::Result<bool> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            match key.code {
                KeyCode::Esc => app.close_playlist_menu(),
                KeyCode::Up | KeyCode::Char('k') => app.playlist_menu_up(),
                KeyCode::Down | KeyCode::Char('j') => app.playlist_menu_down(),
                KeyCode::Enter => app.playlist_menu_select(),
                _ => {}
            }
        }
        Event::Mouse(mouse) => {
            if let Some(ref menu) = app.playlist_menu {
                let width: u16 = 25;
                let height = (menu.items.len() + 2) as u16;
                let popup_x = (size.width.saturating_sub(width)) / 2;
                let popup_y = (size.height.saturating_sub(height)) / 2;

                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if mouse.column >= popup_x && mouse.column < popup_x + width
                            && mouse.row >= popup_y && mouse.row < popup_y + height
                        {
                            let item_row = mouse.row.saturating_sub(popup_y + 1);
                            if (item_row as usize) < menu.items.len() {
                                app.playlist_menu.as_mut().unwrap().selected = item_row as usize;
                                app.playlist_menu_select();
                            }
                        } else {
                            app.close_playlist_menu();
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_rename_playlist_event(app: &mut App, event: &Event, size: Size) -> io::Result<bool> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            match key.code {
                KeyCode::Esc => app.close_playlist_menu(),
                KeyCode::Enter => app.rename_playlist_confirm(),
                KeyCode::Backspace => app.rename_playlist_backspace(),
                KeyCode::Char(c) => app.rename_playlist_input(c),
                _ => {}
            }
        }
        Event::Mouse(mouse) => {
            let width: u16 = 40;
            let height: u16 = 3;
            let popup_x = (size.width.saturating_sub(width)) / 2;
            let popup_y = (size.height.saturating_sub(height)) / 2;

            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if !(mouse.column >= popup_x && mouse.column < popup_x + width
                        && mouse.row >= popup_y && mouse.row < popup_y + height)
                    {
                        app.close_playlist_menu();
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_help_event(app: &mut App, event: &Event) -> io::Result<bool> {
    match event {
        Event::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            // Any key closes help
            app.close_help();
        }
        Event::Mouse(mouse) => {
            // Any click closes help
            if matches!(mouse.kind, MouseEventKind::Down(_)) {
                app.close_help();
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_mouse(app: &mut App, kind: MouseEventKind, col: u16, row: u16, size: Size) {
    // Layout:
    // Row 0-2: Now playing (height 3)
    // Row 3-4: Progress bar (height 2)
    // Row 5 to (height-2): Main content
    // Last row: Help bar (height 1)

    let main_content_start = 5u16;
    let main_content_end = size.height.saturating_sub(1);

    // Navigation panel is 20 columns wide (including border)
    let nav_panel_width = 20u16;

    match kind {
        MouseEventKind::Down(button) => {
            // Check if click is in main content area
            if row >= main_content_start && row < main_content_end {
                let content_row = row - main_content_start;

                if col < nav_panel_width {
                    // Click in navigation panel
                    app.active_panel = Panel::Navigation;
                    // Account for border (1 row)
                    if content_row >= 1 {
                        let nav_idx = (content_row - 1) as usize;
                        if nav_idx < app.nav_item_count() {
                            app.nav_state.select(Some(nav_idx));
                            if button == MouseButton::Left {
                                app.select_nav();
                            } else if button == MouseButton::Right {
                                // Right-click on a playlist (index >= 4 means it's a playlist)
                                let fixed_count = 4; // All Tracks, Queue, Recently Played, Recently Added
                                if nav_idx >= fixed_count {
                                    let playlist_idx = nav_idx - fixed_count;
                                    if let Some(playlist) = app.playlists.get(playlist_idx) {
                                        app.open_playlist_menu(playlist.id, playlist.name.clone());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Click in track list panel
                    app.active_panel = Panel::TrackList;
                    // Account for border (1 row) and header (2 rows with margin)
                    if content_row >= 3 {
                        let visible_idx = (content_row - 3) as usize;
                        // Convert visible index to actual index accounting for scroll
                        let actual_idx = app.scroll_offset + visible_idx;
                        let len = if app.is_searching && !app.filtered_indices.is_empty() {
                            app.filtered_indices.len()
                        } else {
                            app.displayed_items.len()
                        };
                        if actual_idx < len {
                            // Get the real track index (accounting for search filtering)
                            let track_idx = if app.is_searching && !app.filtered_indices.is_empty() {
                                app.filtered_indices.get(actual_idx).copied()
                            } else {
                                Some(actual_idx)
                            };

                            if button == MouseButton::Left {
                                app.track_state.select(Some(actual_idx));

                                // Check for double-click
                                if let Some(idx) = track_idx {
                                    let now = std::time::Instant::now();
                                    let is_double_click = app.last_click
                                        .map(|(time, last_idx)| {
                                            last_idx == idx && now.duration_since(time).as_millis() < 400
                                        })
                                        .unwrap_or(false);

                                    if is_double_click {
                                        app.play_selected();
                                        app.last_click = None;
                                    } else {
                                        app.last_click = Some((now, idx));
                                    }
                                }
                            } else if button == MouseButton::Right {
                                // Right click opens context menu
                                if let Some(idx) = track_idx {
                                    app.open_context_menu(idx);
                                }
                            }
                        }
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => {
            // Scroll based on mouse position, not active panel
            if col < nav_panel_width && row >= main_content_start && row < main_content_end {
                app.nav_down();
            } else {
                app.track_down();
            }
        }
        MouseEventKind::ScrollUp => {
            // Scroll based on mouse position, not active panel
            if col < nav_panel_width && row >= main_content_start && row < main_content_end {
                app.nav_up();
            } else {
                app.track_up();
            }
        }
        _ => {}
    }
}
