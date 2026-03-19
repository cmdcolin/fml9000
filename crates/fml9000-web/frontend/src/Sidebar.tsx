import { createSignal, createResource, createEffect, For, Show } from "solid-js";
import type { NavItem, SidebarData, TrackItem } from "./types";
import {
  selectedSources, setSelectedSources, isSourceSelected,
  setTracks, setFilteredIndices, setSearchQuery,
} from "./state";
import { get, post, del, put } from "./api";
import { showContextMenu } from "./util";
import { updateUrlParams } from "./App";
import styles from "./Sidebar.module.css";

function urlForItem(item: NavItem): string | undefined {
  if (item.kind === "auto") return `/api/sources/${item.id}`;
  if (item.kind === "playlist" && item.db_id != null) return `/api/playlists/${item.db_id}/items`;
  if (item.kind === "channel" && item.db_id != null) return `/api/youtube/channels/${item.db_id}/videos`;
  return undefined;
}

async function fetchItemsForSource(item: NavItem): Promise<TrackItem[]> {
  const url = urlForItem(item);
  if (!url) return [];
  const resp = await fetch(url);
  return resp.json();
}

export function Sidebar() {
  const [data, { refetch }] = createResource(
    () => get("sidebar") as Promise<SidebarData>
  );
  const [multiSelect, setMultiSelect] = createSignal(false);
  let restored = false;

  function allItems(): NavItem[] {
    const d = data();
    if (!d) return [];
    return [...d.auto_playlists, ...d.user_playlists, ...d.youtube_channels];
  }

  async function loadSelectedItems() {
    const selected = selectedSources();
    const all = allItems();
    const items = all.filter((i) => selected.has(i.id));
    if (items.length === 0) return;

    setSearchQuery("");
    const results = await Promise.all(items.map(fetchItemsForSource));
    const merged: TrackItem[] = results.flat();
    setTracks(merged);
    setFilteredIndices(merged.map((_, i) => i));
  }

  function selectOnly(item: NavItem) {
    setSelectedSources(new Set([item.id]));
    updateUrlParams({ source: item.id });
    loadSelectedItems();
  }

  function toggleSource(item: NavItem, checked: boolean) {
    const next = new Set(selectedSources());
    if (checked) {
      next.add(item.id);
    } else {
      next.delete(item.id);
    }
    setSelectedSources(next);
    updateUrlParams({ source: [...next].join(",") });
    loadSelectedItems();
  }

  createEffect(() => {
    const d = data();
    if (!d || restored) return;
    restored = true;

    // Enable multi-select mode if URL has multiple sources
    if (selectedSources().size > 1) {
      setMultiSelect(true);
    }
    loadSelectedItems();
  });

  return (
    <nav class={styles.sidebar}>
      <div class={styles.toolbar}>
        <button
          class={styles.toolbarBtn}
          classList={{ [styles.toolbarBtnActive]: multiSelect() }}
          onclick={() => setMultiSelect((m) => !m)}
          title="Toggle multi-select"
        >
          {"\u2611"}
        </button>
      </div>
      <Show when={data()} fallback={<div class={styles.loading}>Loading...</div>}>
        {(d) => <>
          <Section title="Auto Playlists" items={d().auto_playlists}
            multiSelect={multiSelect()} onSelect={selectOnly} onToggle={toggleSource} />
          <Section
            title="Playlists" items={d().user_playlists}
            multiSelect={multiSelect()} onSelect={selectOnly} onToggle={toggleSource}
            onAdd={async () => {
              const name = prompt("Playlist name:");
              if (name) { await post("playlists", { name }); refetch(); }
            }}
            onContext={(e, item) => {
              showContextMenu(e, [
                { label: "Rename", action: async () => {
                  const n = prompt("New name:", item.label);
                  if (n && item.db_id != null) { await put(`playlists/${item.db_id}`, { name: n }); refetch(); }
                }},
                { label: "Delete", danger: true, action: async () => {
                  if (confirm(`Delete "${item.label}"?`) && item.db_id != null) { await del(`playlists/${item.db_id}`); refetch(); }
                }},
              ]);
            }}
          />
          <Section
            title="YouTube" items={d().youtube_channels}
            multiSelect={multiSelect()} onSelect={selectOnly} onToggle={toggleSource}
            onAdd={async () => {
              const url = prompt("YouTube channel URL or @handle:");
              if (url) { await post("youtube/channels", { url }); refetch(); }
            }}
            onContext={(e, item) => {
              showContextMenu(e, [
                { label: "Delete", danger: true, action: async () => {
                  if (confirm(`Delete "${item.label}"?`) && item.db_id != null) { await del(`youtube/channels/${item.db_id}`); refetch(); }
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
  multiSelect: boolean;
  onSelect: (item: NavItem) => void;
  onToggle: (item: NavItem, checked: boolean) => void;
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
          <Show when={props.multiSelect} fallback={
            <div
              class={styles.item}
              classList={{ [styles.itemActive]: isSourceSelected(item.id) }}
              onclick={() => props.onSelect(item)}
              oncontextmenu={(e) => {
                if (props.onContext) { e.preventDefault(); props.onContext(e, item); }
              }}
            >
              {item.label}
            </div>
          }>
            <label
              class={styles.item}
              classList={{ [styles.itemActive]: isSourceSelected(item.id) }}
              oncontextmenu={(e) => {
                if (props.onContext) { e.preventDefault(); props.onContext(e, item); }
              }}
            >
              <input
                type="checkbox"
                class={styles.checkbox}
                checked={isSourceSelected(item.id)}
                onchange={(e) => props.onToggle(item, e.currentTarget.checked)}
              />
              <span class={styles.itemLabel}>{item.label}</span>
            </label>
          </Show>
        )}
      </For>
    </div>
  );
}
