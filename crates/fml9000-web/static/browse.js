const Browse = {
  albums: [],
  filteredAlbums: [],
  el: null,
  gridEl: null,
  detailEl: null,
  searchEl: null,

  init() {
    this.el = document.getElementById("browse-container");
    this.gridEl = document.getElementById("browse-grid");
    this.detailEl = document.getElementById("browse-detail");
    this.searchEl = document.getElementById("browse-search");
    this.searchEl.addEventListener("input", (e) => this.filter(e.target.value));

    document.getElementById("browse-back").addEventListener("click", () => {
      this.showGrid();
    });
  },

  async load() {
    const resp = await fetch("/api/albums");
    this.albums = await resp.json();
    this.filteredAlbums = this.albums;
    this.renderGrid();
  },

  filter(query) {
    if (!query) {
      this.filteredAlbums = this.albums;
    } else {
      const q = query.toLowerCase();
      this.filteredAlbums = this.albums.filter(
        (a) =>
          a.album.toLowerCase().includes(q) ||
          a.artist.toLowerCase().includes(q)
      );
    }
    this.renderGrid();
  },

  renderGrid() {
    const frag = document.createDocumentFragment();
    for (const album of this.filteredAlbums) {
      const card = document.createElement("div");
      card.className = "browse-card";

      const img = document.createElement("img");
      img.className = "browse-card-img";
      img.loading = "lazy";
      img.src = `/api/thumbnails/${encodeURIComponent(album.representative_filename)}`;
      img.onerror = function () {
        this.style.display = "none";
      };

      const info = document.createElement("div");
      info.className = "browse-card-info";

      const title = document.createElement("div");
      title.className = "browse-card-title";
      title.textContent = album.album;

      const artist = document.createElement("div");
      artist.className = "browse-card-artist";
      artist.textContent = album.artist;

      info.appendChild(title);
      info.appendChild(artist);
      card.appendChild(img);
      card.appendChild(info);

      card.addEventListener("click", () => this.openAlbum(album));
      frag.appendChild(card);
    }
    this.gridEl.textContent = "";
    this.gridEl.appendChild(frag);
  },

  async openAlbum(album) {
    const resp = await fetch(
      `/api/albums/${encodeURIComponent(album.artist)}/${encodeURIComponent(album.album)}`
    );
    const tracks = await resp.json();

    App.tracks = tracks;
    App.filteredIndices = tracks.map((_, i) => i);

    this.gridEl.style.display = "none";
    this.searchEl.style.display = "none";
    this.detailEl.style.display = "flex";

    document.getElementById("browse-detail-title").textContent = album.album;
    document.getElementById("browse-detail-artist").textContent = album.artist;

    const img = document.getElementById("browse-detail-art");
    img.src = `/api/thumbnails/${encodeURIComponent(album.representative_filename)}`;

    const tableEl = document.getElementById("browse-detail-tracks");
    tableEl.textContent = "";

    for (let i = 0; i < tracks.length; i++) {
      const t = tracks[i];
      const row = document.createElement("div");
      row.className = "detail-track-row";
      row.innerHTML =
        `<span class="detail-track-num">${esc(t.tr) || (i + 1)}</span>` +
        `<span class="detail-track-title">${esc(t.t)}</span>` +
        `<span class="detail-track-dur">${fmtDur(t.d)}</span>`;
      const idx = i;
      row.addEventListener("dblclick", () => {
        App.api("playback/play", { index: idx });
      });
      tableEl.appendChild(row);
    }
  },

  showGrid() {
    this.gridEl.style.display = "grid";
    this.searchEl.style.display = "block";
    this.detailEl.style.display = "none";
  },

  show() {
    this.el.style.display = "flex";
    document.getElementById("table-container").style.display = "none";
    document.getElementById("track-search").style.display = "none";
    App.currentView = "browse";
    if (this.albums.length === 0) {
      this.load();
    }
    this.showGrid();
  },
};
