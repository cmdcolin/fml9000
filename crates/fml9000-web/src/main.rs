mod api;
mod server;
mod state;
mod ws;

use state::{AppState, AudioCommand};

fn refresh_youtube_channels(state: &AppState) {
    let channels = fml9000_core::get_youtube_channels().unwrap_or_default();
    if channels.is_empty() {
        return;
    }

    let now = chrono::Utc::now().naive_utc();
    let min_age = chrono::Duration::hours(6);
    let channels_to_refresh: Vec<_> = channels
        .iter()
        .filter(|c| {
            c.last_fetched
                .map(|lf| now - lf > min_age)
                .unwrap_or(true)
        })
        .collect();

    if channels_to_refresh.is_empty() {
        return;
    }

    println!(
        "Refreshing {} of {} YouTube channels (skipping recently checked)...",
        channels_to_refresh.len(),
        channels.len()
    );

    for (i, channel) in channels_to_refresh.iter().enumerate() {
        // Be polite: wait between channels
        if i > 0 {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }

        let existing = match fml9000_core::get_video_ids_for_channel(channel.id) {
            Ok(ids) => ids,
            Err(e) => {
                eprintln!("  Failed to get video IDs for {}: {e}", channel.name);
                continue;
            }
        };

        let playlist_id = match fml9000_core::get_playlist_id_for_handle(
            channel.handle.as_deref().unwrap_or(&channel.channel_id),
        ) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("  Failed to get playlist ID for {}: {e}", channel.name);
                continue;
            }
        };

        // Small pause between API calls for same channel
        std::thread::sleep(std::time::Duration::from_secs(2));

        let new_videos = match fml9000_core::fetch_new_videos(
            &playlist_id,
            &existing,
            false,
            |_, _| {},
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  Failed to fetch new videos for {}: {e}", channel.name);
                continue;
            }
        };

        // Always update last_fetched so we don't retry immediately
        if let Err(e) = fml9000_core::update_channel_last_fetched(channel.id) {
            eprintln!("  Failed to update last_fetched: {e}");
        }

        if !new_videos.is_empty() {
            println!("  {} new videos for {}", new_videos.len(), channel.name);

            let videos_for_db: Vec<_> = new_videos
                .iter()
                .map(|v| {
                    (
                        v.video_id.clone(),
                        v.title.clone(),
                        None::<i32>,
                        v.thumbnail_url.clone(),
                        v.published_at,
                    )
                })
                .collect();

            if let Err(e) = fml9000_core::add_youtube_videos(channel.id, &videos_for_db) {
                eprintln!("  Failed to insert videos: {e}");
                continue;
            }

            state
                .new_video_counts
                .lock()
                .unwrap()
                .insert(channel.id, new_videos.len());

            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "youtube_refresh",
                "channel_id": channel.id,
                "channel_name": channel.name,
                "new_count": new_videos.len(),
            })) {
                let _ = state.ws_broadcast.send(json);
            }
        }
    }
    println!("YouTube refresh complete.");
}

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
    *state.volume.lock().unwrap() = settings.volume as f32;
    state.send_audio(AudioCommand::SetVolume(settings.volume as f32));

    // Background art extraction on startup
    tokio::task::spawn_blocking(|| {
        println!("Extracting album art...");
        let (extracted, total) =
            fml9000_core::thumbnail_cache::download_all_album_art(|done, total| {
                if done % 50 == 0 || done == total {
                    println!("  Album art: {done}/{total}");
                }
            });
        println!("Album art: extracted {extracted} new thumbnails from {total} albums");

        println!("Downloading video thumbnails...");
        let (downloaded, total) =
            fml9000_core::thumbnail_cache::download_all_video_thumbnails(|done, total| {
                if done % 50 == 0 || done == total {
                    println!("  Video thumbnails: {done}/{total}");
                }
            });
        println!("Video thumbnails: downloaded {downloaded} new from {total} videos");
    });

    // Background YouTube channel refresh (every 6 hours, with polite delays)
    let refresh_state = state.clone();
    tokio::spawn(async move {
        // Wait 2 minutes after startup before first check
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        loop {
            let rs = refresh_state.clone();
            tokio::task::spawn_blocking(move || refresh_youtube_channels(&rs)).await.ok();
            // Check every 6 hours
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    });

    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
        loop {
            interval.tick().await;
            tick_state.tick();
            tick_state.broadcast_state();
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
