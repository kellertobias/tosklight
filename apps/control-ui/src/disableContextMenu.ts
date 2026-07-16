export function enableSetOnContextMenu(target: Document = document): () => void {
  const pendingClicks = new Set<number>();
  const view = target.defaultView ?? window;
  const setOnContextMenu = (event: Event) => {
    event.preventDefault();
    event.stopPropagation();
    const clickTarget = event.target;
    if (!(clickTarget instanceof Element)) return;
    const setClickTarget = clickTarget.closest("[data-set-click-target]") ?? clickTarget;

    view.dispatchEvent(new CustomEvent("light:desk-action", { detail: "set" }));
    const timer = view.setTimeout(() => {
      pendingClicks.delete(timer);
      const click = new MouseEvent("click", {
        bubbles: true,
        cancelable: true,
      });
      Object.defineProperty(click, "lightSetShortcut", { value: true });
      setClickTarget.dispatchEvent(click);
    });
    pendingClicks.add(timer);
  };

  target.addEventListener("contextmenu", setOnContextMenu, { capture: true });
  return () => {
    target.removeEventListener("contextmenu", setOnContextMenu, { capture: true });
    pendingClicks.forEach((timer) => view.clearTimeout(timer));
    pendingClicks.clear();
  };
}

export function isSetContextClick(event: Event): boolean {
  return Boolean((event as Event & { lightSetShortcut?: boolean }).lightSetShortcut);
}
