import { createSignal, createMemo, For, Show } from "solid-js";
import type { AlbumItem, TrackItem } from "./types";
import { tracks, setTracks, setFilteredIndices, activeNavId } from "./state";
import { get, post } from "./api";
import { fmtDur } from "./util";
import styles from "./Browse.module.css";

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

export function Browse() {
  const [query, setQuery] = createSignal("");
  const [detail, setDetail] = createSignal<AlbumDetail | null>(null);
  const [imgErrors, setImgErrors] = createSignal<Set<string>>(new Set());

  function onImgError(key: string) {
    setImgErrors((prev) => new Set(prev).add(key));
  }

  const isAlbumView = createMemo(() => {
    const nav = activeNavId();
    return !nav || nav === "all_tracks" || nav === "all_media";
  });

  const items = createMemo(() => {
    const q = query().toLowerCase();
    if (isAlbumView()) {
      return [];
    }
    const all = tracks();
    if (!q) return all;
    return all.filter(
      (t) =>
        (t.t && t.t.toLowerCase().includes(q)) ||
        (t.ar && t.ar.toLowerCase().includes(q)) ||
        (t.al && t.al.toLowerCase().includes(q))
    );
  });

  async function openAlbum(album: AlbumItem) {
    const resp = await fetch(
      `/api/albums/${encodeURIComponent(album.artist)}/${encodeURIComponent(album.album)}`
    );
    const trackList: TrackItem[] = await resp.json();
    setTracks(trackList);
    setFilteredIndices(trackList.map((_, i) => i));
    setDetail({ album, tracks: trackList });
  }

  function playTrackItem(idx: number) {
    post("playback/play", { index: idx });
  }

  return (
    <div class={styles.container}>
      <Show when={!detail()}>
        <input
          type="text" class={styles.search} placeholder="Search..."
          value={query()} oninput={(e) => setQuery(e.currentTarget.value)}
        />
        <Show when={isAlbumView()}>
          <AlbumGrid
            query={query()}
            imgErrors={imgErrors()}
            onImgError={onImgError}
            onOpen={openAlbum}
          />
        </Show>
        <Show when={!isAlbumView()}>
          <div class={styles.grid}>
            <For each={items()}>
              {(item, idx) => {
                const imgSrc = item.video_id
                  ? videoThumbnailUrl(item.video_id)
                  : item.f
                    ? thumbnailUrl(item.f)
                    : undefined;
                const key = item.video_id ?? item.f ?? "";
                return (
                  <div class={styles.card} ondblclick={() => playTrackItem(idx())}>
                    <Show
                      when={imgSrc && !imgErrors().has(key)}
                      fallback={<div class={styles.placeholder}>{"\u266B"}</div>}
                    >
                      <img
                        class={styles.cardImg} loading="lazy"
                        src={imgSrc}
                        onerror={() => onImgError(key)}
                      />
                    </Show>
                    <div class={styles.cardInfo}>
                      <div class={styles.cardTitle}>{item.t ?? "Unknown"}</div>
                      <div class={styles.cardArtist}>{item.ar ?? ""}</div>
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </Show>
      <Show when={detail()}>
        {(d) => (
          <div style={{ padding: "16px", "overflow-y": "auto", flex: "1" }}>
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

function AlbumGrid(props: {
  query: string;
  imgErrors: Set<string>;
  onImgError: (key: string) => void;
  onOpen: (album: AlbumItem) => void;
}) {
  const [albums, setAlbums] = createSignal<AlbumItem[]>([]);

  // Fetch once on first render
  (async () => {
    const data: AlbumItem[] = await get("albums");
    setAlbums(data);
  })();

  const filtered = createMemo(() => {
    const q = props.query.toLowerCase();
    const all = albums();
    if (!q) return all;
    return all.filter(
      (a) => a.album.toLowerCase().includes(q) || a.artist.toLowerCase().includes(q)
    );
  });

  return (
    <div class={styles.grid}>
      <For each={filtered()}>
        {(album) => (
          <div class={styles.card} onclick={() => props.onOpen(album)}>
            <Show
              when={!props.imgErrors.has(album.representative_filename)}
              fallback={<div class={styles.placeholder}>{"\u266B"}</div>}
            >
              <img
                class={styles.cardImg} loading="lazy"
                src={thumbnailUrl(album.representative_filename)}
                onerror={() => props.onImgError(album.representative_filename)}
              />
            </Show>
            <div class={styles.cardInfo}>
              <div class={styles.cardTitle}>{album.album}</div>
              <div class={styles.cardArtist}>{album.artist}</div>
            </div>
          </div>
        )}
      </For>
    </div>
  );
}
