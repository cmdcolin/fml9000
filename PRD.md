# FML9000 — Product Requirements & Next Steps

## Current State

FML9000 is a Rust music player inspired by foobar2000 with three interfaces (GTK4
GUI, TUI, CLI scanner) sharing a SQLite database. It supports local audio playback
via rodio, YouTube video/audio via yt-dlp + GStreamer, playlists, queue management,
library scanning, and a thumbnail browse grid.

## Architecture Improvements

Move completed items to COMPLETED.md

### Modularize TUI app.rs (1461 lines)

`app.rs` handles input, state, playback, mpv IPC, YouTube fetching, and playlist
management in a single struct. Extract into:

- `tui/mpv.rs` — mpv process spawning, IPC socket communication, pause/resume/seek
- `tui/youtube_fetch.rs` — channel fetching with mpsc progress reporting
- `tui/input.rs` — key event handling (the large `match` block)

This mirrors how the GTK crate splits concerns across modules.

### Modularize preferences_dialog.rs (1098 lines)

The YouTube channel refresh logic (~400 lines of progress dialog + background
thread + progress polling) should move to its own module, e.g.
`youtube_refresh.rs`. The preferences dialog would call into it.

### Share more playback logic between GTK and TUI

The `PlaybackController` (GTK) and `App` (TUI) both manage play statistics
tracking (50% threshold counting), album art display, and now-playing state. A
shared `PlaybackState` struct in core could track current item, play stats, and
threshold logic.

## Feature Roadmap

### P0 — Polish & Reliability

- **MPRIS/D-Bus integration** — expose playback state to the Linux desktop so
  media keys, KDE Connect, and status widgets work. The `mpris-server` crate
  provides a clean async API for this.

- **Gapless playback** — rodio supports appending sources to a sink before the
  current one finishes. Pre-decode the next track and queue it when the current
  track is ~5 seconds from ending.

- **Rescan file watcher** — the GTK app imports `notify` but doesn't use it for
  live library updates. Wire the `notify` watcher to detect new/deleted files in
  configured folders and update the DB + stores automatically.

### P1 — Browse Tab Enhancements

- **Lazy thumbnail loading indicator** — show a spinner or placeholder icon while
  thumbnails are being fetched, instead of a blank space.

- **Thumbnail grid responsiveness** — currently the grid auto-sizes columns
  between 2-20. Add a zoom slider or use the window width to compute a sensible
  default column count.

- **Album detail view** — clicking an album card in the browse grid could expand
  to show all tracks in that album, with play-all and queue-all actions.

- **Thumbnail cache management** — add a "Clear thumbnail cache" option in
  preferences. Show cache size.

### P2 — Library & Metadata

- **Smart playlists** — saved queries like "most played", "added this week",
  "unplayed", "genre = jazz". Store the query predicate in the playlists table
  and evaluate on selection.

- **Tag editing** — inline editing of artist/album/title/track number in the
  playlist view. Write back to files via lofty.

- **Lyrics display** — fetch lyrics from embedded tags (USLT/SYLT) or an API,
  display in the Art tab synchronized with playback position.

- **ReplayGain** — read ReplayGain tags and adjust volume per-track for
  consistent loudness. rodio supports `amplify()` on sources.

### P3 — Integrations

- **Last.fm / ListenBrainz scrobbling** — the play statistics threshold (50%)
  already exists. Add an HTTP POST to the scrobble API when the threshold is
  crossed. Store API keys in settings.

- **Discord Rich Presence** — show currently playing track in Discord status.
  The `discord-rich-presence` crate handles the IPC.

- **Import/export playlists** — M3U and XSPF playlist file import/export for
  interop with other players.

### P4 — Audio Processing

- **Equalizer** — a parametric EQ applied to the rodio source chain. Could use
  biquad filters from the `biquad` crate.

- **Crossfade** — fade out the ending track while fading in the next, using
  rodio's `FadeIn`/`TakeDuration` adapters.

- **Playback speed control** — useful for podcasts/audiobooks. rodio supports
  `speed()` on sources.

## Technical Debt

- **Playlist view (775 lines)** has the entire column setup, factory callbacks,
  drag-and-drop, context menus, and right-click handling in one function. Could
  be split into column setup, factory setup, and action setup functions.

- **`add_youtube_videos` tuple parameter** — the function takes
  `&[(String, String, Option<i32>, Option<String>, Option<NaiveDateTime>)]`
  which is unreadable. Should use a struct.

- **Error handling inconsistency** — some DB functions return `Result`, others
  silently `eprintln!` and return defaults. Standardize on `Result` and let
  callers decide how to handle errors.

- **Test coverage** — 58 unit tests cover core logic (facets, audio detection,
  playback navigation, media item accessors). Missing: DB integration tests
  (need temp database), GTK widget tests (need headless display), thumbnail
  cache tests, YouTube API tests (need mocking).

## Next features

- Add easy 'add youtube' button in the 'Browse' panel
