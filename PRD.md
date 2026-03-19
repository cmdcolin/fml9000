# FML9000 — Product Requirements & Next Steps

## Current State

FML9000 is a Rust music player inspired by foobar2000. The primary interface is
a **web UI** (axum server + SolidJS/TypeScript frontend), with legacy GTK4 and
TUI interfaces also available. All share a SQLite database via `fml9000-core`.

The web UI supports: track list with sorting/search, album browse grid with
cover art, sidebar navigation with multi-select, playback controls with
seek/volume/shuffle/repeat, right-click context menus, WebSocket real-time
state, auto-advance, play count tracking, YouTube channel management, album art
extraction, URL state persistence, and keyboard shortcuts.

Audio plays server-side via rodio. The server exposes a REST + WebSocket API.

## Web UI — Priority Roadmap

### P0 — Core Polish

- **YouTube audio playback** — currently only local files play via rodio.
  YouTube items show in the library but can't play in the web UI. Implement
  audio-only playback via yt-dlp piped to rodio (same approach as the GTK
  vaporwave pipeline). Fall back gracefully if yt-dlp is unavailable.

- **Drag-and-drop playlist reordering** — playlists can be created and items
  added/removed, but the order can't be changed in the UI. Add drag handles
  on playlist items in list view.

- **Now-playing highlight in browse view** — the list view highlights the
  playing track, but the browse grid doesn't indicate which card is currently
  playing. Add a subtle overlay or border on the active card.

- **Playback queue UI** — the queue exists in the API but the web UI has no
  dedicated queue view. Add a queue panel or drawer showing upcoming tracks
  with reorder/remove controls.

- **Error feedback** — API errors are silently swallowed. Show toast
  notifications for failures (failed to play, failed to add channel, etc.).

### P1 — Browse & Navigation

- **Artist detail view** — clicking an artist name in the browse grid or table
  shows all albums by that artist with expandable track listings.

- **Thumbnail zoom slider** — let the user control browse card size with a
  slider, adjusting the CSS grid `minmax` value.

- **Keyboard navigation in browse grid** — arrow keys to move between cards,
  Enter to open/play, Escape to go back. Currently mouse-only.

- **Infinite scroll** — replace the "Show more" button with automatic loading
  when the user scrolls near the bottom of the browse grid.

- **Column resizing in list view** — allow dragging column edges to resize.
  Persist column widths to URL params or localStorage.

- **Column visibility toggle** — let users show/hide columns (track number,
  genre, etc.) from a column picker dropdown.

### P2 — Library & Metadata

- **Smart playlists** — saved queries like "most played", "added this week",
  "unplayed", "genre = jazz". Store the query in the playlists table and
  evaluate server-side.

- **Inline tag editing** — click-to-edit artist/album/title in the list view.
  Write back to files via lofty on the server.

- **Rescan from web UI** — trigger a library rescan from the web interface
  with progress feedback, instead of requiring `fml9000-scan` CLI.

- **Lyrics display** — fetch from embedded tags or an API, display in a panel
  synchronized with playback position.

- **ReplayGain / volume normalization** — read ReplayGain tags and adjust
  volume per-track via rodio's `amplify()`.

### P3 — Integrations

- **Last.fm / ListenBrainz scrobbling** — the 50% play threshold already
  exists. Add an HTTP POST to the scrobble API when crossed. Store API keys
  in settings.

- **Import/export playlists** — M3U and XSPF playlist file import/export.

- **Discord Rich Presence** — show currently playing track in Discord status.

- **MPRIS/D-Bus integration** — expose playback state for media keys and
  desktop widget integration.

### P4 — Audio & Playback

- **Gapless playback** — pre-decode next track and queue to rodio sink before
  current finishes.

- **Crossfade** — fade out ending track while fading in next using rodio's
  `FadeIn`/`TakeDuration` adapters.

- **Equalizer** — parametric EQ via biquad filters in the rodio source chain.

- **Playback speed** — useful for podcasts/audiobooks. rodio supports
  `speed()` on sources.

- **Audio device selection** — pick output device from a web preferences panel.

### P5 — YouTube

- **YouTube playlist import** — accept playlist URLs (not just channels) and
  import all videos.

- **YouTube search** — search YouTube from the browse panel and add individual
  videos without needing a channel URL.

- **Offline audio cache** — download YouTube audio to a local cache so
  previously played videos work offline.

- **YouTube channel refresh** — add a refresh button per channel in the
  sidebar to fetch new videos without re-adding.

## Web UI — Technical Improvements

- **Virtualized list view** — the track table currently renders all rows. For
  libraries over ~10k tracks, add virtualization (only render visible rows).
  SolidJS makes this straightforward with `createVirtualizer` or a custom
  scroll handler.

- **Service worker / offline** — cache the built assets for instant load.
  Could also cache API responses for offline browsing (read-only).

- **Mobile responsive layout** — the sidebar should collapse to a hamburger
  menu on narrow screens. Browse grid already auto-sizes.

- **Dark/light theme toggle** — the CSS variables make this easy. Add a toggle
  in the header and persist to localStorage.

- **WebSocket reconnection indicator** — show a subtle banner when the
  WebSocket disconnects and is reconnecting.

- **Typed API layer** — the `api.ts` functions return `any`. Generate types
  from the Rust API (via OpenAPI spec or a shared schema) for end-to-end
  type safety.

- **Context menu as Solid component** — currently uses raw DOM manipulation.
  Rewrite as a Solid component with portal for proper lifecycle management.

## Legacy Frontends (Lower Priority)

### GTK4

- Modularize `preferences_dialog.rs` (1098 lines) — extract YouTube refresh
- Adopt GTK4 property bindings incrementally to replace `Rc<RefCell<>>` hacks
- Share `PlaybackState` between GTK and web via core crate

### TUI

- Modularize `app.rs` (1461 lines) into mpv, input, and youtube_fetch modules
- Add browse panel with text-based album navigation
- Add playlist management (create/rename/delete)

## Technical Debt

- **`add_youtube_videos` tuple parameter** — takes a 5-element tuple. Should
  use a struct.

- **Error handling inconsistency** — some DB functions return `Result`, others
  silently `eprintln!`. Standardize on `Result`.

- **Test coverage** — 58 unit tests cover core logic. Missing: DB integration
  tests, API endpoint tests, frontend component tests.

- **Context menu cleanup** — the imperative `showContextMenu` in `util.ts`
  creates DOM nodes outside Solid's control. Should be a proper Solid
  component.
