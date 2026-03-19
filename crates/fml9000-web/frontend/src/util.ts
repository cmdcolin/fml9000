export function fmtDur(secs: number | null | undefined) {
  if (secs == null) return "?:??";
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s < 10 ? "0" : ""}${s}`;
}

export interface MenuItem {
  label: string;
  action: () => void;
  danger?: boolean;
}

export function showContextMenu(event: MouseEvent, items: MenuItem[]) {
  const existing = document.querySelector(".context-menu");
  if (existing) existing.remove();

  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.style.left = event.pageX + "px";
  menu.style.top = event.pageY + "px";

  for (const item of items) {
    const el = document.createElement("div");
    el.className = "context-menu-item" + (item.danger ? " danger" : "");
    el.textContent = item.label;
    el.addEventListener("click", () => {
      menu.remove();
      item.action();
    });
    menu.appendChild(el);
  }

  document.body.appendChild(menu);
  const dismiss = (e: Event) => {
    if (!menu.contains(e.target as Node)) {
      menu.remove();
      document.removeEventListener("click", dismiss);
    }
  };
  setTimeout(() => document.addEventListener("click", dismiss), 0);
}
