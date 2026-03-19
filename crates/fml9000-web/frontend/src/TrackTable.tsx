import { For, createMemo } from "solid-js";
import type { TrackItem, PlaylistInfo } from "./types";
import {
  tracks, filteredIndices, searchQuery,
  filterTracks, sortTracks, sortCol, sortAsc, playbackState,
} from "./state";
import { post, get } from "./api";
import { fmtDur, showContextMenu } from "./util";
import styles from "./TrackTable.module.css";

interface Column {
  key: keyof TrackItem;
  label: string;
  flex: string;
  dim?: boolean;
  fmt?: (v: string | number | null | undefined) => string;
}

const COLUMNS: Column[] = [
  { key: "t", label: "Title", flex: "3fr" },
  { key: "ar", label: "Artist", flex: "2fr" },
  { key: "al", label: "Album", flex: "2fr" },
  { key: "tr", label: "Track", flex: "60px", dim: true },
  { key: "g", label: "Genre", flex: "1fr", dim: true },
  { key: "d", label: "Duration", flex: "70px", dim: true, fmt: (v) => fmtDur(v as number | null) },
  { key: "pc", label: "Plays", flex: "60px", dim: true, fmt: (v) => (v ? String(v) : "") },
  { key: "lp", label: "Last Played", flex: "90px", dim: true },
];

const GRID_COLS = COLUMNS.map((c) => c.flex).join(" ");

function trackContextMenu(e: MouseEvent, track: TrackItem) {
  e.preventDefault();
  showContextMenu(e, [
    {
      label: "Add to Queue",
      action: () => {
        if (track.f) post("queue", { track_filename: track.f });
        else if (track.video_db_id) post("queue", { youtube_video_id: track.video_db_id });
      },
    },
    {
      label: "Add to Playlist...",
      action: async () => {
        const playlists: PlaylistInfo[] = await get("playlists");
        if (playlists.length === 0) { alert("No playlists yet."); return; }
        showContextMenu(e, playlists.map((pl) => ({
          label: pl.name,
          action: () => {
            const body: Record<string, unknown> = {};
            if (track.f) body.track_filename = track.f;
            else if (track.video_db_id) body.youtube_video_id = track.video_db_id;
            post(`playlists/${pl.id}/items`, body);
          },
        })));
      },
    },
  ]);
}

export function TrackTable() {
  const rows = createMemo(() => {
    const items = tracks();
    return filteredIndices().map((idx) => ({ idx, track: items[idx] }));
  });

  const playingIndex = createMemo(() => playbackState()?.current_index ?? null);

  return (
    <div class={styles.container}>
      <div class={styles.header} style={{ "grid-template-columns": GRID_COLS }}>
        <For each={COLUMNS}>
          {(col) => (
            <div
              class={styles.th}
              classList={{
                [styles.sortAsc]: sortCol() === col.key && sortAsc(),
                [styles.sortDesc]: sortCol() === col.key && !sortAsc(),
              }}
              onclick={() => sortTracks(col.key)}
            >
              {col.label}
            </div>
          )}
        </For>
      </div>
      <div class={styles.scroll}>
        <For each={rows()}>
          {(row) => (
            <div
              class={styles.row}
              classList={{ [styles.playing]: playingIndex() === row.idx }}
              style={{ "grid-template-columns": GRID_COLS }}
              ondblclick={() => post("playback/play", { index: row.idx })}
              oncontextmenu={(e) => trackContextMenu(e, row.track)}
            >
              <For each={COLUMNS}>
                {(col) => (
                  <div class={styles.cell} classList={{ [styles.cellDim]: !!col.dim }}>
                    {col.fmt ? col.fmt(row.track[col.key]) : String(row.track[col.key] ?? "")}
                  </div>
                )}
              </For>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

export function SearchBar() {
  return (
    <div class={styles.searchBar}>
      <input
        type="text"
        class={styles.searchInput}
        placeholder="Search tracks..."
        value={searchQuery()}
        oninput={(e) => filterTracks(e.currentTarget.value)}
      />
      <span class={styles.trackCount}>{filteredIndices().length} items</span>
    </div>
  );
}
