import { createPortal } from "react-dom";
import { isValidElement, type ReactNode } from "react";
import {
  ModalRegistration,
  type ModalClosePolicy,
  type ModalRegistrationElementProps,
} from "../modals/ModalStack";

/**
 * Keeps screen-centered dialogs outside panes and scaled application canvases.
 * Anchored popovers and dropdowns should use their own portal and positioning.
 */
export function ModalPortal({
  children,
  id,
  policy,
  onClose,
}: {
  children: ReactNode;
  id?: string;
  policy?: ModalClosePolicy;
  onClose?: () => void;
}) {
  if (!onClose) return createPortal(children, document.body);
  if (!isValidElement<ModalRegistrationElementProps>(children)) {
    throw new Error("A registered ModalPortal requires one modal layer element");
  }
  return createPortal(
    <ModalRegistration id={id} policy={policy} onClose={onClose}>
      {children}
    </ModalRegistration>,
    document.body,
  );
}

/** Keeps an explicitly non-modal popover inside the active modal stacking context. */
export function activeModalPortalRoot(): Element {
  return document.querySelector('.ui-modal-stack-layer[data-modal-top="true"]')
    ?? document.body;
}
