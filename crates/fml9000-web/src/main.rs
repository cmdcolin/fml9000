mod api;
mod server;
mod state;
mod ws;

use state::AppState;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    println!("Initializing database...");
    if let Err(e) = fml9000_core::init_db() {
        eprintln!("Failed to initialize database: {e}");
        std::process::exit(1);
    }

    let settings: fml9000_core::CoreSettings = fml9000_core::settings::read_settings();

    let state = AppState::new();
    state
        .shuffle_enabled
        .store(settings.shuffle_enabled, std::sync::atomic::Ordering::Relaxed);
    *state.repeat_mode.lock().unwrap() = settings.repeat_mode;

    let broadcast_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
        loop {
            interval.tick().await;
            broadcast_state.broadcast_state();
        }
    });

    let app = server::create_router(state);

    let addr = format!("0.0.0.0:{port}");
    println!("fml9000-web listening on http://localhost:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server error");
}
