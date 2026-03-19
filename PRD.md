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

- **Now-playing highlight in browse view** — the list view highlights the
  playing track, but the browse grid doesn't indicate which card is currently
  playing. Add a subtle overlay or border on the active card.

- **Playback queue UI** — the queue exists in the API but the web UI has no
  dedicated queue view. Add a queue panel or tab showing upcoming tracks
  with remove controls.

- **Error feedback** — API errors are silently swallowed. Show toast
  notifications for failures (failed to play, failed to add channel, etc.).

- **Drag-and-drop playlist reordering** — playlists can be created and items
  added/removed, but the order can't be changed in the UI.

### P1 — Browse & Navigation

- **Thumbnail zoom slider** — let the user control browse card size with a
  slider, adjusting the CSS grid `minmax` value.

- **Keyboard navigation in browse grid** — arrow keys to move between cards,
  Enter to open/play, Escape to go back.

- **Infinite scroll** — replace the "Show more" button with automatic loading
  when the user scrolls near the bottom of the browse grid.

- **Column resizing in list view** — allow dragging column edges to resize.

- **Column visibility toggle** — let users show/hide columns from a picker.

### P2 — Library & Metadata

- **Smart playlists** — saved queries like "most played", "added this week",
  "unplayed", "genre = jazz". Store the query in the playlists table and
  evaluate server-side.

- **Rescan from web UI** — trigger a library rescan from the web interface
  with progress feedback, instead of requiring `fml9000-scan` CLI.

- **ReplayGain / volume normalization** — read ReplayGain tags and adjust
  volume per-track via rodio's `amplify()`.

### P3 — Audio & Playback

- **Gapless playback** — pre-decode next track and queue to rodio sink before
  current finishes.

- **Crossfade** — fade out ending track while fading in next.

- **Playback speed** — useful for podcasts/audiobooks.

### P4 — YouTube

- **YouTube channel refresh** — add a refresh button per channel in the
  sidebar to fetch new videos without re-adding.

- **YouTube playlist import** — accept playlist URLs (not just channels).

- **YouTube search** — search YouTube from the browse panel and add individual
  videos without needing a channel URL.

- **Offline audio cache** — download YouTube audio to a local cache so
  previously played videos work offline.

## Web UI — Technical Improvements

- **Mobile responsive layout** — the sidebar should collapse to a hamburger
  menu on narrow screens. Browse grid already auto-sizes.

- **Dark/light theme toggle** — the CSS variables make this easy. Add a toggle
  in the header and persist to localStorage.

- **Typed API layer** — the `api.ts` functions return untyped JSON. Generate
  types from the Rust API for end-to-end type safety.

- **Context menu as Solid component** — currently uses raw DOM manipulation.
  Rewrite as a Solid component with portal for proper lifecycle management.

## Legacy Frontends (Lower Priority)

### GTK4

- Modularize `preferences_dialog.rs` (1098 lines) — extract YouTube refresh
- Share `PlaybackState` between GTK and web via core crate

### TUI

- Modularize `app.rs` (1461 lines) into mpv, input, and youtube_fetch modules

## Technical Debt

- **`add_youtube_videos` tuple parameter** — takes a 5-element tuple. Should
  use a struct.

- **Error handling inconsistency** — some DB functions return `Result`, others
  silently `eprintln!`. Standardize on `Result`.

- **Context menu cleanup** — the imperative `showContextMenu` in `util.ts`
  creates DOM nodes outside Solid's control.
