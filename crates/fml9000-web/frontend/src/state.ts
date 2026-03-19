import { createSignal } from "solid-js";
import type { PlaybackState, TrackItem } from "./types";

export const [playbackState, setPlaybackState] = createSignal<PlaybackState | null>(null);
export const [tracks, setTracks] = createSignal<TrackItem[]>([]);
export const [filteredIndices, setFilteredIndices] = createSignal<number[]>([]);
export const [searchQuery, setSearchQuery] = createSignal("");
export const [currentView, setCurrentView] = createSignal<"table" | "browse">("table");
export const [sortCol, setSortCol] = createSignal<keyof TrackItem | null>(null);
export const [sortAsc, setSortAsc] = createSignal(true);
export const [activeNavId, setActiveNavId] = createSignal(
  new URLSearchParams(location.search).get("source") ?? "all_tracks"
);

export function filterTracks(query: string) {
  setSearchQuery(query);
  const items = tracks();
  if (!query) {
    setFilteredIndices(items.map((_, i) => i));
    return;
  }
  const q = query.toLowerCase();
  const result: number[] = [];
  for (let i = 0; i < items.length; i++) {
    const t = items[i];
    if (
      (t.t && t.t.toLowerCase().includes(q)) ||
      (t.ar && t.ar.toLowerCase().includes(q)) ||
      (t.al && t.al.toLowerCase().includes(q)) ||
      (t.g && t.g.toLowerCase().includes(q))
    ) {
      result.push(i);
    }
  }
  setFilteredIndices(result);
}

export function sortTracks(col: keyof TrackItem) {
  if (sortCol() === col) {
    setSortAsc(!sortAsc());
  } else {
    setSortCol(col);
    setSortAsc(true);
  }
  const items = tracks();
  const dir = sortAsc() ? 1 : -1;
  const indices = [...filteredIndices()];
  indices.sort((a, b) => {
    const va = items[a][col] ?? "";
    const vb = items[b][col] ?? "";
    if (typeof va === "number" && typeof vb === "number") {
      return (va - vb) * dir;
    }
    const sa = String(va).toLowerCase();
    const sb = String(vb).toLowerCase();
    if (sa < sb) return -dir;
    if (sa > sb) return dir;
    return 0;
  });
  setFilteredIndices(indices);
}

export function connectWebSocket() {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${proto}//${location.host}/ws`);
  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    if (msg.type === "playback_state") {
      setPlaybackState(msg.data);
    }
  };
  ws.onclose = () => setTimeout(connectWebSocket, 2000);
}
