import { useEffect } from "react";

export function ModalEscapeManager() {
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      const layers = [...document.querySelectorAll<HTMLElement>(".modal-backdrop,.stacked-modal-layer")];
      const top = layers.at(-1);
      if (!top) return;
      const close = top.querySelector<HTMLButtonElement>(".modal-close") ?? top.querySelector<HTMLButtonElement>("button");
      if (!close) return;
      event.preventDefault(); event.stopImmediatePropagation(); close.click();
    };
    window.addEventListener("keydown", keydown, true);
    return () => window.removeEventListener("keydown", keydown, true);
  }, []);
  return null;
}
