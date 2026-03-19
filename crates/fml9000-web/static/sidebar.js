const Sidebar = {
  data: null,
  activeId: "all_tracks",
  el: null,

  init() {
    this.el = document.getElementById("sidebar");
    this.load();
  },

  async load() {
    const resp = await fetch("/api/sidebar");
    this.data = await resp.json();
    this.render();
  },

  render() {
    if (!this.data) return;
    const frag = document.createDocumentFragment();

    frag.appendChild(this.renderSection("Auto Playlists", this.data.auto_playlists));
    frag.appendChild(this.renderSection("Playlists", this.data.user_playlists, true));
    frag.appendChild(this.renderSection("YouTube", this.data.youtube_channels));

    this.el.textContent = "";
    this.el.appendChild(frag);
  },

  renderSection(title, items, showAdd) {
    const section = document.createElement("div");
    section.className = "sidebar-section";

    const header = document.createElement("div");
    header.className = "sidebar-section-header";
    header.textContent = title;

    if (showAdd) {
      const addBtn = document.createElement("button");
      addBtn.className = "sidebar-add-btn";
      addBtn.textContent = "+";
      addBtn.title = "New playlist";
      addBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.createPlaylist();
      });
      header.appendChild(addBtn);
    }

    section.appendChild(header);

    for (const item of items) {
      const el = document.createElement("div");
      el.className = "sidebar-item";
      if (item.id === this.activeId) {
        el.classList.add("active");
      }
      el.textContent = item.label;
      el.dataset.id = item.id;
      el.dataset.kind = item.kind;

      el.addEventListener("click", () => this.selectItem(item));

      if (item.kind === "playlist") {
        el.addEventListener("contextmenu", (e) => {
          e.preventDefault();
          this.showPlaylistContextMenu(e, item);
        });
      }

      section.appendChild(el);
    }

    return section;
  },

  async selectItem(item) {
    this.activeId = item.id;
    this.render();

    App.currentView = "table";
    document.getElementById("browse-container").style.display = "none";
    document.getElementById("table-container").style.display = "flex";
    document.getElementById("track-search").style.display = "flex";

    let url;
    if (item.kind === "auto") {
      url = `/api/sources/${item.id}`;
    } else if (item.kind === "playlist") {
      const id = item.id.replace("playlist_", "");
      url = `/api/playlists/${id}/items`;
    } else if (item.kind === "channel") {
      const id = item.id.replace("channel_", "");
      url = `/api/youtube/channels/${id}/videos`;
    }

    if (url) {
      const resp = await fetch(url);
      const items = await resp.json();
      App.tracks = items;
      App.filteredIndices = items.map((_, i) => i);
      document.getElementById("track-count").textContent = `${items.length} items`;
      document.getElementById("search-input").value = "";
      VirtualTable.setData(items, App.filteredIndices);
    }
  },

  async createPlaylist() {
    const name = prompt("Playlist name:");
    if (!name) return;
    await App.api("playlists", { name });
    this.load();
  },

  showPlaylistContextMenu(event, item) {
    const existing = document.querySelector(".context-menu");
    if (existing) existing.remove();

    const menu = document.createElement("div");
    menu.className = "context-menu";
    menu.style.left = event.pageX + "px";
    menu.style.top = event.pageY + "px";

    const id = parseInt(item.id.replace("playlist_", ""));

    const renameOption = document.createElement("div");
    renameOption.className = "context-menu-item";
    renameOption.textContent = "Rename";
    renameOption.addEventListener("click", async () => {
      menu.remove();
      const newName = prompt("New name:", item.label);
      if (newName) {
        await fetch(`/api/playlists/${id}`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ name: newName }),
        });
        this.load();
      }
    });

    const deleteOption = document.createElement("div");
    deleteOption.className = "context-menu-item danger";
    deleteOption.textContent = "Delete";
    deleteOption.addEventListener("click", async () => {
      menu.remove();
      if (confirm(`Delete playlist "${item.label}"?`)) {
        await fetch(`/api/playlists/${id}`, { method: "DELETE" });
        this.load();
      }
    });

    menu.appendChild(renameOption);
    menu.appendChild(deleteOption);
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
