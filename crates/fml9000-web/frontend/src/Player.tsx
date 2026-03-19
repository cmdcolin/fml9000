import { createSignal, createMemo, onMount, onCleanup } from "solid-js";
import { playbackState } from "./state";
import { post } from "./api";
import { fmtDur } from "./util";
import styles from "./Player.module.css";

export function Player() {
  const [seeking, setSeeking] = createSignal(false);
  const [volDragging, setVolDragging] = createSignal(false);
  let seekRef!: HTMLInputElement;

  function togglePlay() {
    const s = playbackState();
    if (!s) return;
    if (s.playing) {
      post("playback/pause");
    } else if (s.paused) {
      post("playback/resume");
    }
  }

  function onSeekChange() {
    setSeeking(false);
    const s = playbackState();
    if (s?.duration_secs) {
      post("playback/seek", {
        position_secs: (Number(seekRef.value) / 1000) * s.duration_secs,
      });
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if ((e.target as HTMLElement).tagName === "INPUT") return;
    if (e.key === " ") { e.preventDefault(); togglePlay(); }
    else if (e.key === "n") post("playback/next");
    else if (e.key === "p") post("playback/prev");
    else if (e.key === "s") post("playback/stop");
  }

  onMount(() => document.addEventListener("keydown", onKeyDown));
  onCleanup(() => document.removeEventListener("keydown", onKeyDown));

  const seekValue = createMemo(() => {
    if (seeking()) return Number(seekRef?.value ?? 0);
    const s = playbackState();
    if (s && s.duration_secs && s.duration_secs > 0) {
      return Math.round((s.position_secs / s.duration_secs) * 1000);
    }
    return 0;
  });

  const volumeValue = createMemo(() => {
    if (volDragging()) return -1;
    return Math.round((playbackState()?.volume ?? 1) * 100);
  });

  const currentTime = createMemo(() =>
    fmtDur(Math.floor(playbackState()?.position_secs ?? 0))
  );
  const totalTime = createMemo(() => {
    const d = playbackState()?.duration_secs;
    return d ? fmtDur(Math.floor(d)) : "0:00";
  });
  const npTitle = createMemo(() => playbackState()?.current_track?.title ?? "-");
  const npArtist = createMemo(() => playbackState()?.current_track?.artist ?? "-");
  const npAlbum = createMemo(() => playbackState()?.current_track?.album ?? "-");
  const isPlaying = createMemo(() => playbackState()?.playing ?? false);
  const shuffleActive = createMemo(() => playbackState()?.shuffle_enabled ?? false);
  const repeatMode = createMemo(() => playbackState()?.repeat_mode ?? "all");
  const repeatActive = createMemo(() => repeatMode() !== "off");

  return (
    <header class={styles.header}>
      <div class={styles.transport}>
        <button onclick={() => post("playback/prev")} title="Previous">{"\u23EE"}</button>
        <button onclick={togglePlay} title="Play/Pause">
          {isPlaying() ? "\u23F8" : "\u25B6"}
        </button>
        <button onclick={() => post("playback/stop")} title="Stop">{"\u23F9"}</button>
        <button onclick={() => post("playback/next")} title="Next">{"\u23ED"}</button>
      </div>
      <div class={styles.nowPlaying}>
        <span class={styles.npTitle}>{npTitle()}</span>
        <span class={styles.npDim}>{npArtist()}</span>
        <span class={styles.npDim}>{npAlbum()}</span>
      </div>
      <div class={styles.seekBar}>
        <span class={styles.time}>{currentTime()}</span>
        <input
          type="range" min="0" max="1000"
          ref={seekRef}
          prop:value={seekValue()}
          onmousedown={() => setSeeking(true)}
          oninput={() => setSeeking(true)}
          onchange={onSeekChange}
        />
        <span class={styles.time}>{totalTime()}</span>
      </div>
      <div class={styles.controlsRight}>
        <button
          classList={{ [styles.active]: shuffleActive() }}
          onclick={() => post("playback/shuffle", { enabled: !shuffleActive() })}
          title="Shuffle"
        >{"\uD83D\uDD00"}</button>
        <button
          classList={{ [styles.active]: repeatActive() }}
          onclick={() => {
            const modes = ["off", "all", "one"] as const;
            const cur = modes.indexOf(repeatMode() as typeof modes[number]);
            post("playback/repeat", { mode: modes[(cur + 1) % 3] });
          }}
          title="Repeat"
        >{repeatMode() === "one" ? "\uD83D\uDD02" : "\uD83D\uDD01"}</button>
        <input
          type="range" class={styles.volume} min="0" max="100"
          prop:value={volumeValue() >= 0 ? volumeValue() : 100}
          onmousedown={() => setVolDragging(true)}
          oninput={(e) => {
            setVolDragging(true);
            post("playback/volume", { volume: Number(e.currentTarget.value) / 100 });
          }}
          onchange={() => setVolDragging(false)}
          title="Volume"
        />
      </div>
    </header>
  );
}
