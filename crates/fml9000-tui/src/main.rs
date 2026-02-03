mod app;
mod ui;

use app::{App, Panel};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, layout::Size, Terminal};
use std::io;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, Level};

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
    let mut event_stream = EventStream::new();
    let mut tick_interval = interval(Duration::from_millis(500));

    info!("Entering main loop");

    loop {
        // Draw UI
        debug!("Drawing UI...");
        terminal.draw(|f| ui::ui(f, app)).map_err(|e| {
            debug!("Draw error: {:?}", e);
            e
        })?;
        debug!("Draw complete");

        // Wait for either an event or a tick
        debug!("Waiting for event or tick...");
        tokio::select! {
            _ = tick_interval.tick() => {
                debug!("Tick");
                app.on_tick();
            }
            maybe_event = event_stream.next() => {
                debug!("Got event");
                let event = match maybe_event {
                    Some(Ok(event)) => event,
                    Some(Err(_)) => continue,
                    None => break,
                };

                // Handle mouse events
                if let Event::Mouse(mouse) = event {
                    // Ignore mouse move events - they flood in constantly
                    if matches!(mouse.kind, MouseEventKind::Moved) {
                        continue;
                    }
                    debug!("Mouse event: {:?}", mouse.kind);
                    let size = terminal.size()?;
                    handle_mouse(app, mouse.kind, mouse.column, mouse.row, size);
                    continue;
                }

                let key = match event {
                    Event::Key(k) => k,
                    Event::Resize(w, h) => {
                        debug!("Resize event: {}x{}", w, h);
                        continue;
                    }
                    _ => {
                        debug!("Other event type");
                        continue;
                    }
                };

                debug!("Key event: {:?} kind={:?}", key.code, key.kind);

                // Only handle key press events, not releases
                if key.kind != KeyEventKind::Press {
                    debug!("Ignoring non-press key event");
                    continue;
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
                    continue;
                }

                // Normal mode key handling
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('/') => app.start_search(),
                    KeyCode::Tab => app.toggle_panel(),
                    KeyCode::Char(' ') => {
                        if app.now_playing.is_some() {
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
        }
    }

    Ok(())
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
        MouseEventKind::Down(_) => {
            // Check if click is in main content area
            if row >= main_content_start && row < main_content_end {
                let content_row = row - main_content_start;

                if col < nav_panel_width {
                    // Click in navigation panel
                    app.active_panel = Panel::Navigation;
                    // Account for border (1 row)
                    if content_row >= 1 {
                        let nav_idx = (content_row - 1) as usize;
                        if nav_idx < app.nav_items().len() {
                            app.nav_state.select(Some(nav_idx));
                            app.select_nav();
                        }
                    }
                } else {
                    // Click in track list panel
                    app.active_panel = Panel::TrackList;
                    // Account for border (1 row) and header (2 rows with margin)
                    if content_row >= 3 {
                        let track_idx = (content_row - 3) as usize;
                        let len = if app.is_searching && !app.filtered_indices.is_empty() {
                            app.filtered_indices.len()
                        } else {
                            app.displayed_items.len()
                        };
                        if track_idx < len {
                            app.track_state.select(Some(track_idx));
                        }
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if app.active_panel == Panel::Navigation {
                app.nav_down();
            } else {
                app.track_down();
            }
        }
        MouseEventKind::ScrollUp => {
            if app.active_panel == Panel::Navigation {
                app.nav_up();
            } else {
                app.track_up();
            }
        }
        _ => {}
    }
}
