const Player = {
  seeking: false,

  init() {
    document.getElementById("btn-prev").addEventListener("click", () => {
      App.api("playback/prev");
    });
    document.getElementById("btn-play").addEventListener("click", () => {
      this.togglePlay();
    });
    document.getElementById("btn-stop").addEventListener("click", () => {
      App.api("playback/stop");
    });
    document.getElementById("btn-next").addEventListener("click", () => {
      App.api("playback/next");
    });

    const seekEl = document.getElementById("seek");
    seekEl.addEventListener("mousedown", () => { this.seeking = true; });
    seekEl.addEventListener("input", () => { this.seeking = true; });
    seekEl.addEventListener("change", () => {
      this.seeking = false;
      if (App.state && App.state.duration_secs) {
        const pos = (seekEl.value / 1000) * App.state.duration_secs;
        App.api("playback/seek", { position_secs: pos });
      }
    });

    const volEl = document.getElementById("volume");
    volEl.addEventListener("input", () => {
      App.api("playback/volume", { volume: volEl.value / 100 });
    });

    document.getElementById("btn-shuffle").addEventListener("click", () => {
      const enabled = !(App.state && App.state.shuffle_enabled);
      App.api("playback/shuffle", { enabled });
    });

    document.getElementById("btn-repeat").addEventListener("click", () => {
      if (!App.state) return;
      const modes = ["off", "all", "one"];
      const cur = modes.indexOf(App.state.repeat_mode);
      const next = modes[(cur + 1) % modes.length];
      App.api("playback/repeat", { mode: next });
    });
  },

  togglePlay() {
    if (!App.state) return;
    if (App.state.playing) {
      App.api("playback/pause");
    } else if (App.state.paused) {
      App.api("playback/resume");
    }
  },

  update(state) {
    const playBtn = document.getElementById("btn-play");
    playBtn.textContent = state.playing ? "\u23F8" : "\u25B6";

    if (state.current_track) {
      document.getElementById("np-title").textContent = state.current_track.title || "-";
      document.getElementById("np-artist").textContent = state.current_track.artist || "-";
      document.getElementById("np-album").textContent = state.current_track.album || "-";
    } else {
      document.getElementById("np-title").textContent = "-";
      document.getElementById("np-artist").textContent = "-";
      document.getElementById("np-album").textContent = "-";
    }

    if (!this.seeking) {
      const seekEl = document.getElementById("seek");
      if (state.duration_secs && state.duration_secs > 0) {
        seekEl.value = Math.round((state.position_secs / state.duration_secs) * 1000);
      } else {
        seekEl.value = 0;
      }
    }

    document.getElementById("time-current").textContent = fmtDur(Math.floor(state.position_secs));
    document.getElementById("time-total").textContent =
      state.duration_secs ? fmtDur(Math.floor(state.duration_secs)) : "0:00";

    const shuffleBtn = document.getElementById("btn-shuffle");
    shuffleBtn.classList.toggle("active", state.shuffle_enabled);

    const repeatBtn = document.getElementById("btn-repeat");
    repeatBtn.classList.toggle("active", state.repeat_mode !== "off");
    if (state.repeat_mode === "one") {
      repeatBtn.textContent = "\uD83D\uDD02";
    } else {
      repeatBtn.textContent = "\uD83D\uDD01";
    }
  },
};
