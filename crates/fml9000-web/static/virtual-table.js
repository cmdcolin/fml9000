const VirtualTable = {
  ROW_HEIGHT: 28,
  BUFFER: 20,
  scrollEl: null,
  spacerEl: null,
  rowsEl: null,
  tracks: [],
  indices: [],
  renderedStart: -1,
  renderedEnd: -1,
  sortCol: null,
  sortAsc: true,

  init() {
    this.scrollEl = document.getElementById("table-scroll");
    this.spacerEl = document.getElementById("table-spacer");
    this.rowsEl = document.getElementById("table-rows");

    this.scrollEl.addEventListener("scroll", () => this.render());

    document.querySelectorAll(".th").forEach((th) => {
      th.addEventListener("click", () => this.sort(th.dataset.col));
    });

    document.getElementById("search-input").addEventListener("input", (e) => {
      App.filterTracks(e.target.value);
    });

    window.addEventListener("resize", () => this.render());

    document.addEventListener("keydown", (e) => {
      if (e.target.tagName === "INPUT") return;
      switch (e.key) {
        case " ":
          e.preventDefault();
          Player.togglePlay();
          break;
        case "n":
          App.api("playback/next");
          break;
        case "p":
          App.api("playback/prev");
          break;
        case "s":
          App.api("playback/stop");
          break;
      }
    });
  },

  setData(tracks, indices) {
    this.tracks = tracks;
    this.indices = indices;
    this.renderedStart = -1;
    this.renderedEnd = -1;
    this.spacerEl.style.height = indices.length * this.ROW_HEIGHT + "px";
    this.scrollEl.scrollTop = 0;
    this.render();
  },

  render() {
    const scrollTop = this.scrollEl.scrollTop;
    const viewHeight = this.scrollEl.clientHeight;
    const totalRows = this.indices.length;

    let start = Math.floor(scrollTop / this.ROW_HEIGHT) - this.BUFFER;
    let end = Math.ceil((scrollTop + viewHeight) / this.ROW_HEIGHT) + this.BUFFER;
    if (start < 0) start = 0;
    if (end > totalRows) end = totalRows;

    if (start === this.renderedStart && end === this.renderedEnd) return;
    this.renderedStart = start;
    this.renderedEnd = end;

    const fragment = document.createDocumentFragment();
    const playingIndex = App.state ? App.state.current_index : null;

    for (let i = start; i < end; i++) {
      const trackIdx = this.indices[i];
      const t = this.tracks[trackIdx];
      const row = document.createElement("div");
      row.className = "row";
      if (playingIndex === trackIdx) {
        row.classList.add("playing");
      }
      row.style.transform = `translateY(${i * this.ROW_HEIGHT}px)`;
      row.style.position = "absolute";
      row.style.left = "0";
      row.style.right = "0";
      row.dataset.index = trackIdx;

      row.innerHTML =
        `<div class="cell">${esc(t.t)}</div>` +
        `<div class="cell">${esc(t.ar)}</div>` +
        `<div class="cell">${esc(t.al)}</div>` +
        `<div class="cell cell-dim">${esc(t.tr)}</div>` +
        `<div class="cell cell-dim">${esc(t.g)}</div>` +
        `<div class="cell cell-dim">${fmtDur(t.d)}</div>` +
        `<div class="cell cell-dim">${t.pc || ""}</div>` +
        `<div class="cell cell-dim">${esc(t.lp)}</div>`;

      row.addEventListener("dblclick", () => {
        App.api("playback/play", { index: trackIdx });
      });

      row.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        this.showContextMenu(e, t);
      });

      fragment.appendChild(row);
    }

    this.rowsEl.textContent = "";
    this.rowsEl.appendChild(fragment);
  },

  updatePlayingRow() {
    const playingIndex = App.state ? App.state.current_index : null;
    const rows = this.rowsEl.children;
    for (let i = 0; i < rows.length; i++) {
      const row = rows[i];
      const idx = parseInt(row.dataset.index);
      if (idx === playingIndex) {
        row.classList.add("playing");
      } else {
        row.classList.remove("playing");
      }
    }
  },

  sort(col) {
    document.querySelectorAll(".th").forEach((th) => {
      th.classList.remove("sort-asc", "sort-desc");
    });

    if (this.sortCol === col) {
      this.sortAsc = !this.sortAsc;
    } else {
      this.sortCol = col;
      this.sortAsc = true;
    }

    const th = document.querySelector(`.th[data-col="${col}"]`);
    th.classList.add(this.sortAsc ? "sort-asc" : "sort-desc");

    const tracks = this.tracks;
    const dir = this.sortAsc ? 1 : -1;

    this.indices.sort((a, b) => {
      let va = tracks[a][col];
      let vb = tracks[b][col];
      if (va == null) va = "";
      if (vb == null) vb = "";
      if (typeof va === "number" && typeof vb === "number") {
        return (va - vb) * dir;
      }
      va = String(va).toLowerCase();
      vb = String(vb).toLowerCase();
      if (va < vb) return -dir;
      if (va > vb) return dir;
      return 0;
    });

    this.renderedStart = -1;
    this.renderedEnd = -1;
    this.render();
  },

  showContextMenu(event, track) {
    const existing = document.querySelector(".context-menu");
    if (existing) existing.remove();

    const menu = document.createElement("div");
    menu.className = "context-menu";
    menu.style.left = event.pageX + "px";
    menu.style.top = event.pageY + "px";

    const addToQueue = document.createElement("div");
    addToQueue.className = "context-menu-item";
    addToQueue.textContent = "Add to Queue";
    addToQueue.addEventListener("click", () => {
      menu.remove();
      if (track.f) {
        App.api("queue", { track_filename: track.f });
      } else if (track.video_db_id) {
        App.api("queue", { youtube_video_id: track.video_db_id });
      }
    });
    menu.appendChild(addToQueue);

    const addToPlaylist = document.createElement("div");
    addToPlaylist.className = "context-menu-item";
    addToPlaylist.textContent = "Add to Playlist...";
    addToPlaylist.addEventListener("click", async () => {
      menu.remove();
      const resp = await fetch("/api/playlists");
      const playlists = await resp.json();

      if (playlists.length === 0) {
        alert("No playlists yet. Create one in the sidebar first.");
        return;
      }

      const submenu = document.createElement("div");
      submenu.className = "context-menu";
      submenu.style.left = event.pageX + "px";
      submenu.style.top = event.pageY + "px";

      for (const pl of playlists) {
        const item = document.createElement("div");
        item.className = "context-menu-item";
        item.textContent = pl.name;
        item.addEventListener("click", () => {
          submenu.remove();
          const body = {};
          if (track.f) body.track_filename = track.f;
          else if (track.video_db_id) body.youtube_video_id = track.video_db_id;
          fetch(`/api/playlists/${pl.id}/items`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
          });
        });
        submenu.appendChild(item);
      }

      document.body.appendChild(submenu);
      const dismiss2 = (e) => {
        if (!submenu.contains(e.target)) {
          submenu.remove();
          document.removeEventListener("click", dismiss2);
        }
      };
      setTimeout(() => document.addEventListener("click", dismiss2), 0);
    });
    menu.appendChild(addToPlaylist);

    document.body.appendChild(menu);

    const dismiss = (e) => {
      if (!menu.contains(e.target)) {
        menu.remove();
        document.removeEventListener("click", dismiss);
      }
    };
    setTimeout(() => document.addEventListener("click", dismiss), 0);
  },
};

function esc(s) {
  if (s == null) return "";
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function fmtDur(secs) {
  if (secs == null) return "?:??";
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s < 10 ? "0" : ""}${s}`;
}
