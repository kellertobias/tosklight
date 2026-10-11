import { ModalProvider } from "@tosklight/ui/modals";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { DesktopProvider } from "../../platform/desktop";
import { browserDesktopBridge } from "../../platform/desktop/browserDesktopBridge";
import { QuitConfirmOverlay } from "./QuitConfirmOverlay";

it("cancelling quit from one window clears every window and permits a new request", async () => {
  const requested = new Set<() => void>();
  const cancelled = new Set<() => void>();
  const subscribe = async (listeners: Set<() => void>, handler: () => void) => {
    listeners.add(handler);
    return () => { listeners.delete(handler); };
  };
  const bridge = {
    ...browserDesktopBridge,
    available: true,
    cancelQuit: vi.fn(async () => { for (const handler of cancelled) handler(); }),
    onQuitRequested: (handler: () => void) => subscribe(requested, handler),
    onQuitCancelled: (handler: () => void) => subscribe(cancelled, handler),
  };
  const window = () => render(<DesktopProvider bridge={bridge}><ModalProvider><QuitConfirmOverlay /></ModalProvider></DesktopProvider>);
  const main = window();
  const secondary = window();
  await act(async () => {});
  act(() => { for (const handler of requested) handler(); });
  expect(screen.getAllByRole("heading", { name: "Really quit?" })).toHaveLength(2);
  fireEvent.keyDown(document, { key: "Escape" });
  await waitFor(() => expect(screen.queryAllByRole("heading", { name: "Really quit?" })).toHaveLength(0));
  expect(bridge.cancelQuit).toHaveBeenCalledOnce();
  act(() => { for (const handler of requested) handler(); });
  expect(screen.getAllByRole("heading", { name: "Really quit?" })).toHaveLength(2);
  main.unmount();
  secondary.unmount();
  expect(requested.size).toBe(0);
  expect(cancelled.size).toBe(0);
});
