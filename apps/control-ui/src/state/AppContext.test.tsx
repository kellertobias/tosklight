import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProvider, useApp } from "./AppContext";

function ModalState() {
  const { state } = useApp();
  return <><span>{state.systemControlsOpen ? "running-open" : "running-closed"}</span><span>built-in-{state.builtIn ?? "none"}</span></>;
}

const values = new Map<string, string>();
beforeEach(() => {
  values.clear();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
    clear: () => values.clear(),
  });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

describe("desk shortcuts", () => {
  it("opens the running menu for hardware Shift Clear", () => {
    render(<AppProvider><ModalState/></AppProvider>);
    expect(screen.getByText("running-closed")).toBeInTheDocument();

    act(() => window.dispatchEvent(new CustomEvent("light:desk-action", { detail: "shift-clear" })));

    expect(screen.getByText("running-open")).toBeInTheDocument();
  });

  it("keeps hardware Shift 0 unassigned while retaining the operator Help shortcut", () => {
    render(<AppProvider><ModalState/></AppProvider>);

    act(() => window.dispatchEvent(new CustomEvent("light:desk-action", { detail: "shift-0" })));
    expect(screen.getByText("built-in-none")).toBeInTheDocument();

    act(() => window.dispatchEvent(new CustomEvent("light:desk-action", { detail: "shift-9" })));
    expect(screen.getByText("built-in-help")).toBeInTheDocument();
  });
});
