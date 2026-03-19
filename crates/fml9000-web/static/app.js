const App = {
  ws: null,
  state: null,
  tracks: [],
  filteredIndices: [],
  selectedIndex: null,
  currentView: "table",

  init() {
    this.connectWebSocket();
    this.loadTracks();
    Player.init();
    VirtualTable.init();
    Sidebar.init();
    Browse.init();

    document.getElementById("tab-list").addEventListener("click", () => {
      this.showListView();
    });
    document.getElementById("tab-browse").addEventListener("click", () => {
      Browse.show();
      document.getElementById("tab-list").classList.remove("active");
      document.getElementById("tab-browse").classList.add("active");
    });
  },

  showListView() {
    this.currentView = "table";
    document.getElementById("browse-container").style.display = "none";
    document.getElementById("table-container").style.display = "flex";
    document.getElementById("track-search").style.display = "flex";
    document.getElementById("tab-list").classList.add("active");
    document.getElementById("tab-browse").classList.remove("active");
  },

  connectWebSocket() {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    this.ws = new WebSocket(`${proto}//${location.host}/ws`);
    this.ws.onmessage = (e) => {
      const msg = JSON.parse(e.data);
      if (msg.type === "playback_state") {
        this.state = msg.data;
        Player.update(msg.data);
        VirtualTable.updatePlayingRow();
      }
    };
    this.ws.onclose = () => {
      setTimeout(() => this.connectWebSocket(), 2000);
    };
  },

  async loadTracks() {
    const resp = await fetch("/api/tracks");
    this.tracks = await resp.json();
    this.filteredIndices = this.tracks.map((_, i) => i);
    document.getElementById("track-count").textContent =
      `${this.tracks.length} tracks`;
    VirtualTable.setData(this.tracks, this.filteredIndices);
  },

  filterTracks(query) {
    if (!query) {
      this.filteredIndices = this.tracks.map((_, i) => i);
    } else {
      const q = query.toLowerCase();
      this.filteredIndices = [];
      for (let i = 0; i < this.tracks.length; i++) {
        const t = this.tracks[i];
        if (
          (t.t && t.t.toLowerCase().includes(q)) ||
          (t.ar && t.ar.toLowerCase().includes(q)) ||
          (t.al && t.al.toLowerCase().includes(q)) ||
          (t.g && t.g.toLowerCase().includes(q))
        ) {
          this.filteredIndices.push(i);
        }
      }
    }
    document.getElementById("track-count").textContent =
      `${this.filteredIndices.length} tracks`;
    VirtualTable.setData(this.tracks, this.filteredIndices);
  },

  async api(path, body) {
    const opts = { method: "POST", headers: { "Content-Type": "application/json" } };
    if (body !== undefined) {
      opts.body = JSON.stringify(body);
    }
    const resp = await fetch(`/api/${path}`, opts);
    return resp.json();
  },
};

document.addEventListener("DOMContentLoaded", () => App.init());
