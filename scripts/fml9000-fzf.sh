#!/usr/bin/env bash
#
# fml9000-fzf.sh - Browse and play fml9000 library with fzf + mpv
#
# Dependencies: sqlite3, fzf, mpv
#
# Usage:
#   fml9000-fzf.sh                # Interactive menu (pick a category, then browse)
#   fml9000-fzf.sh tracks         # Browse local tracks only
#   fml9000-fzf.sh videos         # Browse youtube videos only
#   fml9000-fzf.sh playlists      # Browse playlists
#   fml9000-fzf.sh channels       # Browse youtube channels, then their videos
#   fml9000-fzf.sh recent         # Recently played
#   fml9000-fzf.sh export         # Export full library to TSV
#   fml9000-fzf.sh export-tracks  # Export tracks to TSV
#   fml9000-fzf.sh export-videos  # Export videos to TSV

DB="$HOME/.config/fml9000/library.db"

if [ ! -f "$DB" ]; then
  echo "Error: fml9000 database not found at $DB" >&2
  exit 1
fi

for cmd in sqlite3 fzf mpv; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "Error: $cmd is required but not found" >&2
    exit 1
  fi
done

format_duration() {
  local secs="$1"
  if [ -z "$secs" ] || [ "$secs" = "" ]; then
    echo "--:--"
  else
    printf "%d:%02d" $((secs / 60)) $((secs % 60))
  fi
}

play_selection() {
  local selection="$1"
  local type="${selection%%	*}"
  local uri="${selection#*	}"
  # uri is the second tab-separated field

  if [ "$type" = "track" ]; then
    mpv --no-video "$uri"
  elif [ "$type" = "video" ]; then
    mpv "https://www.youtube.com/watch?v=$uri"
  fi
}

browse_all() {
  {
    sqlite3 -separator $'\t' "$DB" "
      SELECT 'track', filename,
        COALESCE(artist, '?') || ' - ' || COALESCE(title, REPLACE(filename, RTRIM(filename, REPLACE(filename, '/', '')), '')) ||
        ' [' || COALESCE(CAST(duration_seconds/60 AS TEXT) || ':' || SUBSTR('0' || CAST(duration_seconds%60 AS TEXT), -2), '--:--') || ']' ||
        ' {' || COALESCE(album, '?') || '}'
      FROM tracks ORDER BY artist, album, track;
    "
    sqlite3 -separator $'\t' "$DB" "
      SELECT 'video', video_id,
        '[YT] ' || COALESCE(
          (SELECT name FROM youtube_channels WHERE youtube_channels.id = youtube_videos.channel_id), '?'
        ) || ' - ' || title ||
        ' [' || COALESCE(CAST(duration_seconds/60 AS TEXT) || ':' || SUBSTR('0' || CAST(duration_seconds%60 AS TEXT), -2), '--:--') || ']'
      FROM youtube_videos ORDER BY published_at DESC;
    "
  } | fzf --delimiter=$'\t' --with-nth=3 \
      --preview='echo {}' \
      --header='Enter: play with mpv | Ctrl-C: quit' \
      --bind='enter:accept' | while IFS=$'\t' read -r type uri _display; do
    play_selection "$type	$uri"
  done
}

browse_tracks() {
  sqlite3 -separator $'\t' "$DB" "
    SELECT 'track', filename,
      COALESCE(artist, '?') || ' - ' || COALESCE(title, REPLACE(filename, RTRIM(filename, REPLACE(filename, '/', '')), '')) ||
      ' [' || COALESCE(CAST(duration_seconds/60 AS TEXT) || ':' || SUBSTR('0' || CAST(duration_seconds%60 AS TEXT), -2), '--:--') || ']' ||
      ' {' || COALESCE(album, '?') || '}'
    FROM tracks ORDER BY artist, album, track;
  " | fzf --delimiter=$'\t' --with-nth=3 \
      --header='Enter: play with mpv | Ctrl-C: quit' \
      --bind='enter:accept' | while IFS=$'\t' read -r type uri _display; do
    play_selection "$type	$uri"
  done
}

browse_videos() {
  sqlite3 -separator $'\t' "$DB" "
    SELECT 'video', video_id,
      '[YT] ' || COALESCE(
        (SELECT name FROM youtube_channels WHERE youtube_channels.id = youtube_videos.channel_id), '?'
      ) || ' - ' || title ||
      ' [' || COALESCE(CAST(duration_seconds/60 AS TEXT) || ':' || SUBSTR('0' || CAST(duration_seconds%60 AS TEXT), -2), '--:--') || ']'
    FROM youtube_videos ORDER BY published_at DESC;
  " | fzf --delimiter=$'\t' --with-nth=3 \
      --header='Enter: play with mpv | Ctrl-C: quit' \
      --bind='enter:accept' | while IFS=$'\t' read -r type uri _display; do
    play_selection "$type	$uri"
  done
}

browse_playlists() {
  local playlist
  playlist=$(sqlite3 -separator $'\t' "$DB" "
    SELECT id, name || ' (' || (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_tracks.playlist_id = playlists.id) || ' items)'
    FROM playlists ORDER BY name;
  " | fzf --delimiter=$'\t' --with-nth=2 --header='Select a playlist')

  if [ -z "$playlist" ]; then
    return
  fi

  local playlist_id="${playlist%%	*}"

  {
    sqlite3 -separator $'\t' "$DB" "
      SELECT 'track', t.filename,
        COALESCE(t.artist, '?') || ' - ' || COALESCE(t.title, t.filename) ||
        ' [' || COALESCE(CAST(t.duration_seconds/60 AS TEXT) || ':' || SUBSTR('0' || CAST(t.duration_seconds%60 AS TEXT), -2), '--:--') || ']'
      FROM playlist_tracks pt
      JOIN tracks t ON pt.track_filename = t.filename
      WHERE pt.playlist_id = $playlist_id
      ORDER BY pt.position;
    "
    sqlite3 -separator $'\t' "$DB" "
      SELECT 'video', v.video_id,
        '[YT] ' || v.title ||
        ' [' || COALESCE(CAST(v.duration_seconds/60 AS TEXT) || ':' || SUBSTR('0' || CAST(v.duration_seconds%60 AS TEXT), -2), '--:--') || ']'
      FROM playlist_tracks pt
      JOIN youtube_videos v ON pt.youtube_video_id = v.id
      WHERE pt.playlist_id = $playlist_id
      ORDER BY pt.position;
    "
  } | fzf --delimiter=$'\t' --with-nth=3 \
      --header='Enter: play with mpv | Ctrl-C: quit' \
      --bind='enter:accept' | while IFS=$'\t' read -r type uri _display; do
    play_selection "$type	$uri"
  done
}

browse_recent() {
  {
    sqlite3 -separator $'\t' "$DB" "
      SELECT 'track', filename,
        COALESCE(artist, '?') || ' - ' || COALESCE(title, filename) ||
        ' (played: ' || last_played || ', count: ' || play_count || ')'
      FROM tracks
      WHERE last_played IS NOT NULL
      ORDER BY last_played DESC
      LIMIT 50;
    "
    sqlite3 -separator $'\t' "$DB" "
      SELECT 'video', video_id,
        '[YT] ' || title ||
        ' (played: ' || last_played || ', count: ' || play_count || ')'
      FROM youtube_videos
      WHERE last_played IS NOT NULL
      ORDER BY last_played DESC
      LIMIT 50;
    "
  } | sort -t$'\t' -k3 -r | fzf --delimiter=$'\t' --with-nth=3 \
      --header='Recently played | Enter: play with mpv' \
      --bind='enter:accept' | while IFS=$'\t' read -r type uri _display; do
    play_selection "$type	$uri"
  done
}

export_tracks() {
  sqlite3 -header -separator $'\t' "$DB" "
    SELECT filename, title, artist, album, album_artist, genre, track,
           duration_seconds, play_count, last_played, added
    FROM tracks ORDER BY artist, album, track;
  "
}

export_videos() {
  sqlite3 -header -separator $'\t' "$DB" "
    SELECT v.video_id, v.title, c.name AS channel, v.duration_seconds,
           v.play_count, v.last_played, v.published_at
    FROM youtube_videos v
    LEFT JOIN youtube_channels c ON v.channel_id = c.id
    ORDER BY v.published_at DESC;
  "
}

export_all() {
  echo "=== LOCAL TRACKS ==="
  export_tracks
  echo ""
  echo "=== YOUTUBE VIDEOS ==="
  export_videos
}

browse_channels() {
  local channel
  channel=$(sqlite3 -separator $'\t' "$DB" "
    SELECT id, name || ' (' || COALESCE(handle, '') || ', ' ||
      (SELECT COUNT(*) FROM youtube_videos WHERE youtube_videos.channel_id = youtube_channels.id) || ' videos)'
    FROM youtube_channels ORDER BY name;
  " | fzf --delimiter=$'\t' --with-nth=2 --header='Select a channel')

  if [ -z "$channel" ]; then
    return
  fi

  local channel_id="${channel%%	*}"

  sqlite3 -separator $'\t' "$DB" "
    SELECT 'video', video_id,
      title ||
      ' [' || COALESCE(CAST(duration_seconds/60 AS TEXT) || ':' || SUBSTR('0' || CAST(duration_seconds%60 AS TEXT), -2), '--:--') || ']' ||
      ' (' || COALESCE(published_at, '') || ')'
    FROM youtube_videos
    WHERE channel_id = $channel_id
    ORDER BY published_at DESC;
  " | fzf --delimiter=$'\t' --with-nth=3 \
      --header='Enter: play with mpv | Ctrl-C: quit' \
      --bind='enter:accept' | while IFS=$'\t' read -r type uri _display; do
    play_selection "$type	$uri"
  done
}

interactive_menu() {
  while true; do
    local choice
    choice=$(printf '%s\n' \
      "All Media" \
      "Tracks" \
      "Videos" \
      "Playlists" \
      "YouTube Channels" \
      "Recently Played" \
      | fzf --header='fml9000 - Select a category' --no-info)

    if [ -z "$choice" ]; then
      break
    fi

    case "$choice" in
      "All Media")         browse_all ;;
      "Tracks")            browse_tracks ;;
      "Videos")            browse_videos ;;
      "Playlists")         browse_playlists ;;
      "YouTube Channels")  browse_channels ;;
      "Recently Played")   browse_recent ;;
    esac
  done
}

case "${1:-}" in
  tracks)        browse_tracks ;;
  videos)        browse_videos ;;
  playlists)     browse_playlists ;;
  channels)      browse_channels ;;
  recent)        browse_recent ;;
  export)        export_all ;;
  export-tracks) export_tracks ;;
  export-videos) export_videos ;;
  "")            interactive_menu ;;
  *)             echo "Unknown command: $1" >&2; exit 1 ;;
esac
