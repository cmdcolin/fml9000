import { createResource, createEffect, For, Show } from "solid-js";
import type { NavItem, SidebarData, TrackItem } from "./types";
import {
  activeNavId, setActiveNavId, setTracks,
  setFilteredIndices, setSearchQuery,
} from "./state";
import { get, post, del, put } from "./api";
import { showContextMenu } from "./util";
import { updateUrlParams } from "./App";
import styles from "./Sidebar.module.css";

async function fetchSidebar(): Promise<SidebarData> {
  return get("sidebar");
}

export function Sidebar() {
  const [data, { refetch }] = createResource(fetchSidebar);

  async function selectItem(item: NavItem) {
    console.log("[Sidebar] selectItem:", item.id, item.kind, item.label);
    setActiveNavId(item.id);
    setSearchQuery("");
    updateUrlParams({ source: item.id });

    let url: string | undefined;
    if (item.kind === "auto") url = `/api/sources/${item.id}`;
    else if (item.kind === "playlist") url = `/api/playlists/${item.id.replace("playlist_", "")}/items`;
    else if (item.kind === "channel") url = `/api/youtube/channels/${item.id.replace("channel_", "")}/videos`;

    if (url) {
      console.log("[Sidebar] fetching:", url);
      const items: TrackItem[] = await (await fetch(url)).json();
      console.log("[Sidebar] loaded", items.length, "items");
      setTracks(items);
      setFilteredIndices(items.map((_: TrackItem, i: number) => i));
    }
  }

  // Restore source from URL once sidebar data loads
  createEffect(() => {
    const d = data();
    if (!d) return;
    const params = new URLSearchParams(location.search);
    const source = params.get("source") ?? "all_tracks";
    const all = [...d.auto_playlists, ...d.user_playlists, ...d.youtube_channels];
    const item = all.find((i) => i.id === source);
    if (item) {
      selectItem(item);
    }
  });

  return (
    <nav class={styles.sidebar}>
      <Show when={data()} fallback={<div class={styles.loading}>Loading...</div>}>
        {(d) => <>
          <Section title="Auto Playlists" items={d().auto_playlists} onSelect={selectItem} />
          <Section
            title="Playlists" items={d().user_playlists} onSelect={selectItem}
            onAdd={async () => {
              const name = prompt("Playlist name:");
              if (name) { await post("playlists", { name }); refetch(); }
            }}
            onContext={(e, item) => {
              const id = parseInt(item.id.replace("playlist_", ""));
              showContextMenu(e, [
                { label: "Rename", action: async () => {
                  const n = prompt("New name:", item.label);
                  if (n) { await put(`playlists/${id}`, { name: n }); refetch(); }
                }},
                { label: "Delete", danger: true, action: async () => {
                  if (confirm(`Delete "${item.label}"?`)) { await del(`playlists/${id}`); refetch(); }
                }},
              ]);
            }}
          />
          <Section
            title="YouTube" items={d().youtube_channels} onSelect={selectItem}
            onAdd={async () => {
              const url = prompt("YouTube channel URL or @handle:");
              if (url) { await post("youtube/channels", { url }); refetch(); }
            }}
            onContext={(e, item) => {
              const id = parseInt(item.id.replace("channel_", ""));
              showContextMenu(e, [
                { label: "Delete", danger: true, action: async () => {
                  if (confirm(`Delete "${item.label}"?`)) { await del(`youtube/channels/${id}`); refetch(); }
                }},
              ]);
            }}
          />
        </>}
      </Show>
    </nav>
  );
}

interface SectionProps {
  title: string;
  items: NavItem[];
  onSelect: (item: NavItem) => void;
  onAdd?: () => void;
  onContext?: (e: MouseEvent, item: NavItem) => void;
}

function Section(props: SectionProps) {
  return (
    <div class={styles.section}>
      <div class={styles.sectionHeader}>
        {props.title}
        <Show when={props.onAdd}>
          <button class={styles.addBtn} onclick={props.onAdd}>+</button>
        </Show>
      </div>
      <For each={props.items}>
        {(item) => (
          <div
            class={styles.item}
            classList={{ [styles.itemActive]: activeNavId() === item.id }}
            onclick={() => props.onSelect(item)}
            oncontextmenu={(e) => {
              if (props.onContext) { e.preventDefault(); props.onContext(e, item); }
            }}
          >
            {item.label}
          </div>
        )}
      </For>
    </div>
  );
}
