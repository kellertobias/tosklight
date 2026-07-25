import { createPortal } from "react-dom";
import type { ReactNode } from "react";

/**
 * Keeps screen-centered dialogs outside panes and scaled application canvases.
 * Anchored popovers and dropdowns should use their own portal and positioning.
 */
export function ModalPortal({ children }: { children: ReactNode }) {
  return createPortal(children, document.body);
}
