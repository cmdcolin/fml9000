import { onMount, Show } from "solid-js";
import { currentView, setCurrentView, connectWebSocket } from "./state";
import { Player } from "./Player";
import { Sidebar } from "./Sidebar";
import { TrackTable, SearchBar } from "./TrackTable";
import { Browse } from "./Browse";
import { InputDialog } from "./InputDialog";
import styles from "./App.module.css";
import "./style.css";

export function updateUrlParams(updates: Record<string, string>) {
  const params = new URLSearchParams(location.search);
  for (const [k, v] of Object.entries(updates)) {
    params.set(k, v);
  }
  history.replaceState(null, "", `?${params.toString()}`);
}

export function App() {
  onMount(() => {
    connectWebSocket();

    const params = new URLSearchParams(location.search);
    const view = params.get("view");
    if (view === "browse") {
      setCurrentView("browse");
    }
    // Sidebar handles initial data load via URL source param
  });

  function switchView(view: "table" | "browse") {
    setCurrentView(view);
    updateUrlParams({ view });
  }

  return (
    <>
      <InputDialog />
      <Player />
      <div class={styles.body}>
        <Sidebar />
        <div class={styles.content}>
          <div class={styles.viewTabs}>
            <button
              class={styles.viewTab}
              classList={{ [styles.viewTabActive!]: currentView() === "table" }}
              onclick={() => switchView("table")}
            >List</button>
            <button
              class={styles.viewTab}
              classList={{ [styles.viewTabActive!]: currentView() === "browse" }}
              onclick={() => switchView("browse")}
            >Browse</button>
          </div>
          <Show when={currentView() === "table"}>
            <SearchBar />
            <TrackTable />
          </Show>
          <Show when={currentView() === "browse"}>
            <Browse />
          </Show>
        </div>
      </div>
    </>
  );
}
