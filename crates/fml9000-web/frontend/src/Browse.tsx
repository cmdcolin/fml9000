import { createSignal, createMemo, createResource, createEffect, For, Show } from "solid-js";
import type { AlbumItem, TrackItem } from "./types";
import { tracks, setTracks, setFilteredIndices, selectedSources } from "./state";
import { get, post } from "./api";
import { fmtDur } from "./util";
import styles from "./Browse.module.css";

const PAGE_SIZE = 200;

interface AlbumDetail {
  album: AlbumItem;
  tracks: TrackItem[];
}

function thumbnailUrl(filename: string) {
  return `/api/thumbnails?key=${encodeURIComponent(filename)}`;
}

function videoThumbnailUrl(videoId: string) {
  return `https://i.ytimg.com/vi/${videoId}/mqdefault.jpg`;
}

function cardImgSrc(item: TrackItem) {
  if (item.video_id) return videoThumbnailUrl(item.video_id);
  if (item.f) return thumbnailUrl(item.f);
  return undefined;
}

function cardImgKey(item: TrackItem) {
  return item.video_id ?? item.f ?? "";
}

function itemTooltip(item: TrackItem) {
  const lines: string[] = [];
  if (item.t) lines.push(item.t);
  if (item.ar) lines.push(item.ar);
  if (item.al) lines.push(item.al);
  if (item.g) lines.push(item.g);
  if (item.d) lines.push(fmtDur(item.d));
  if (item.pc > 0) lines.push(`${item.pc} play${item.pc === 1 ? "" : "s"}`);
  if (item.lp) lines.push(`Last played: ${item.lp}`);
  return lines.join("\n");
}

export function Browse() {
  const [query, setQuery] = createSignal("");
  const [detail, setDetail] = createSignal<AlbumDetail | null>(null);
  const [imgErrors, setImgErrors] = createSignal<Set<string>>(new Set());
  const [visibleCount, setVisibleCount] = createSignal(PAGE_SIZE);
  const [albums] = createResource(() => get("albums") as Promise<AlbumItem[]>);

  function onImgError(key: string) {
    setImgErrors((prev) => new Set(prev).add(key));
  }

  const ALBUM_SOURCES = new Set(["all-tracks", "all-media"]);
  const isAlbumView = createMemo(() => {
    const sources = selectedSources();
    if (sources.size === 0) return true;
    for (const s of sources) {
      if (!ALBUM_SOURCES.has(s)) return false;
    }
    return true;
  });

  const filteredAlbums = createMemo(() => {
    const q = query().toLowerCase();
    const all = albums() ?? [];
    if (!q) return all;
    return all.filter(
      (a) => a.album.toLowerCase().includes(q) || a.artist.toLowerCase().includes(q)
    );
  });

  const filteredItems = createMemo(() => {
    if (isAlbumView()) return [];
    const q = query().toLowerCase();
    const all = tracks();
    if (!q) return all;
    return all.filter(
      (t) =>
        (t.t && t.t.toLowerCase().includes(q)) ||
        (t.ar && t.ar.toLowerCase().includes(q)) ||
        (t.al && t.al.toLowerCase().includes(q))
    );
  });

  // Reset visible count when data changes
  createEffect(() => {
    filteredAlbums();
    filteredItems();
    setVisibleCount(PAGE_SIZE);
  });

  const visibleAlbums = createMemo(() => filteredAlbums().slice(0, visibleCount()));
  const visibleItems = createMemo(() => filteredItems().slice(0, visibleCount()));
  const totalCount = createMemo(() =>
    isAlbumView() ? filteredAlbums().length : filteredItems().length
  );
  const hasMore = createMemo(() => visibleCount() < totalCount());

  async function openAlbum(album: AlbumItem) {
    const resp = await fetch(
      `/api/albums/${encodeURIComponent(album.artist)}/${encodeURIComponent(album.album)}`
    );
    const trackList: TrackItem[] = await resp.json();
    setTracks(trackList);
    setFilteredIndices(trackList.map((_, i) => i));
    setDetail({ album, tracks: trackList });
  }

  return (
    <div class={styles.container}>
      <Show when={detail()} fallback={
        <>
          <input
            type="text" class={styles.search} placeholder="Search..."
            value={query()} oninput={(e) => setQuery(e.currentTarget.value)}
          />
          <div class={styles.grid}>
            <Show when={isAlbumView()}>
              <For each={visibleAlbums()}>
                {(album) => (
                  <BrowseCard
                    imgSrc={thumbnailUrl(album.representative_filename)}
                    imgKey={album.representative_filename}
                    title={album.album}
                    subtitle={album.artist}
                    tooltip={`${album.album}\n${album.artist}`}
                    imgErrors={imgErrors()}
                    onImgError={onImgError}
                    onclick={() => openAlbum(album)}
                  />
                )}
              </For>
            </Show>
            <Show when={!isAlbumView()}>
              <For each={visibleItems()}>
                {(item, idx) => (
                  <BrowseCard
                    imgSrc={cardImgSrc(item)}
                    imgKey={cardImgKey(item)}
                    title={item.t ?? "Unknown"}
                    subtitle={item.ar ?? ""}
                    tooltip={itemTooltip(item)}
                    imgErrors={imgErrors()}
                    onImgError={onImgError}
                    ondblclick={() => post("playback/play", { index: idx() })}
                  />
                )}
              </For>
            </Show>
          </div>
          <Show when={hasMore()}>
            <div class={styles.showMore}>
              <button class={styles.showMoreBtn} onclick={() => setVisibleCount((c) => c + PAGE_SIZE)}>
                Show more ({totalCount() - visibleCount()} remaining)
              </button>
            </div>
          </Show>
        </>
      }>
        {(d) => (
          <div class={styles.detail}>
            <button class={styles.backBtn} onclick={() => setDetail(null)}>&larr; Back</button>
            <div class={styles.detailHeader}>
              <img class={styles.detailArt} src={thumbnailUrl(d().album.representative_filename)} />
              <div>
                <div class={styles.detailTitle}>{d().album.album}</div>
                <div class={styles.detailArtist}>{d().album.artist}</div>
              </div>
            </div>
            <For each={d().tracks}>
              {(t, i) => (
                <div class={styles.detailRow} ondblclick={() => post("playback/play", { index: i() })}>
                  <span class={styles.detailNum}>{t.tr ?? i() + 1}</span>
                  <span>{t.t}</span>
                  <span class={styles.detailDur}>{fmtDur(t.d)}</span>
                </div>
              )}
            </For>
          </div>
        )}
      </Show>
    </div>
  );
}

function BrowseCard(props: {
  imgSrc?: string;
  imgKey: string;
  title: string;
  subtitle: string;
  tooltip?: string;
  imgErrors: Set<string>;
  onImgError: (key: string) => void;
  onclick?: () => void;
  ondblclick?: () => void;
}) {
  return (
    <div class={styles.card} onclick={props.onclick} ondblclick={props.ondblclick} title={props.tooltip}>
      <div class={styles.imgWrap}>
        <Show
          when={props.imgSrc && !props.imgErrors.has(props.imgKey)}
          fallback={<div class={styles.placeholder}>{"\u266B"}</div>}
        >
          <img
            class={styles.cardImg} loading="lazy"
            src={props.imgSrc}
            onerror={() => props.onImgError(props.imgKey)}
          />
        </Show>
      </div>
      <div class={styles.cardInfo}>
        <div class={styles.cardTitle}>{props.title}</div>
        <div class={styles.cardArtist}>{props.subtitle}</div>
      </div>
    </div>
  );
}
