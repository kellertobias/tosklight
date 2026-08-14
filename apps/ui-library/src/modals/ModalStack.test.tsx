import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Button } from "../controls";
import {
  ModalFrame,
  ModalProvider,
  ModalRegistration,
  useModalStack,
} from "./ModalStack";

afterEach(cleanup);

function NestedHarness({ closeFirst, closeSecond }: { closeFirst: () => void; closeSecond: () => void }) {
  const [second, setSecond] = useState(true);
  return (
    <ModalProvider>
      <ModalFrame id="first" ariaLabel="First" title="First" onClose={closeFirst}>
        <p>First layer</p>
      </ModalFrame>
      {second && (
        <ModalFrame id="second" ariaLabel="Second" title="Second" onClose={() => {
          closeSecond();
          setSecond(false);
        }}>
          <p>Second layer</p>
        </ModalFrame>
      )}
    </ModalProvider>
  );
}

describe("ModalProvider", () => {
  it("routes Escape and backdrop only to the top eligible modal", () => {
    const first = vi.fn();
    const second = vi.fn();
    render(<NestedHarness closeFirst={first} closeSecond={second} />);
    expect(document.querySelector('[data-modal-id="first"]')).toHaveStyle({ "--modal-stack-index": "0" });
    expect(document.querySelector('[data-modal-id="second"]')).toHaveStyle({ "--modal-stack-index": "1" });
    expect(document.querySelector('[data-modal-id="first"]')).toHaveAttribute("data-modal-top", "false");
    expect(document.querySelector('[data-modal-id="second"]')).toHaveAttribute("data-modal-top", "true");
    expect(screen.getByRole("dialog", { name: "Second" })).toHaveAttribute("aria-modal", "true");
    expect(document.querySelector('[data-modal-id="first"] > section')).not.toHaveAttribute("aria-modal");
    const firstLayer = document.querySelector<HTMLElement>('[data-modal-id="first"]')!;
    fireEvent.pointerDown(firstLayer);
    expect(first).not.toHaveBeenCalled();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(second).toHaveBeenCalledOnce();
    expect(first).not.toHaveBeenCalled();
    fireEvent.pointerDown(firstLayer);
    expect(first).toHaveBeenCalledOnce();
  });

  it("honors every close policy", () => {
    const close = vi.fn();
    render(
      <ModalProvider>
        <ModalFrame id="locked" ariaLabel="Locked" title="Locked" policy={{ escape: false, backdrop: false, explicit: false }} onClose={close}>
          Locked workflow
        </ModalFrame>
      </ModalProvider>,
    );
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.pointerDown(document.querySelector('[data-modal-id="locked"]')!);
		expect(screen.getByRole("button", { name: "Close modal" })).toBeDisabled();
    expect(close).not.toHaveBeenCalled();
  });

  it("does not route Escape past a top modal whose Escape policy is disabled", () => {
    const lower = vi.fn();
    const top = vi.fn();
    render(
      <ModalProvider>
        <ModalFrame id="lower" ariaLabel="Lower" title="Lower" onClose={lower}>Lower</ModalFrame>
        <ModalFrame id="top" ariaLabel="Top" title="Top"
          policy={{ escape: false }} onClose={top}>Top</ModalFrame>
      </ModalProvider>,
    );
    fireEvent.keyDown(document, { key: "Escape" });
    expect(top).not.toHaveBeenCalled();
    expect(lower).not.toHaveBeenCalled();
  });

  it("supports programmatic close by stable modal identifier", () => {
    const close = vi.fn();
    function Closer() {
      const stack = useModalStack();
      return <Button onClick={() => stack.close("target")}>Close target</Button>;
    }
    render(
      <ModalProvider>
        <Closer />
        <ModalFrame id="target" ariaLabel="Target" title="Target" onClose={close}>Target content</ModalFrame>
      </ModalProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Close target" }));
    expect(close).toHaveBeenCalledOnce();
  });

  it("registers an existing application dialog without adding wrapper markup", () => {
    const close = vi.fn();
    render(
      <ModalProvider>
        <ModalRegistration id="application-dialog" onClose={close}>
          <div className="stacked-modal-layer">
            <section role="dialog" aria-label="Application dialog">
              <Button>Existing action</Button>
            </section>
          </div>
        </ModalRegistration>
      </ModalProvider>,
    );
    const layer = document.body.querySelector<HTMLElement>(".stacked-modal-layer");
    expect(layer).toHaveClass("stacked-modal-layer");
    expect(layer).toHaveAttribute("data-modal-id", "application-dialog");
    expect(layer).toHaveAttribute("data-modal-top", "true");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  });

  it("routes an application-owned backdrop through the same registered close action", () => {
    const close = vi.fn();
    render(
      <ModalProvider>
        <ModalRegistration id="application-backdrop" onClose={close}>
          <div className="stacked-modal-layer">
            <section role="dialog" aria-label="Application backdrop dialog">
              Existing dialog
            </section>
          </div>
        </ModalRegistration>
      </ModalProvider>,
    );
    const layer = document.body.querySelector<HTMLElement>(
      '[data-modal-id="application-backdrop"]',
    )!;
    fireEvent.pointerDown(screen.getByRole("dialog", { name: "Application backdrop dialog" }));
    expect(close).not.toHaveBeenCalled();
    fireEvent.pointerDown(layer);
    expect(close).toHaveBeenCalledOnce();
  });

  it("does not duplicate an application-owned backdrop handler", () => {
    const close = vi.fn();
    render(
      <ModalProvider>
        <ModalRegistration id="owned-backdrop" onClose={close}>
          <div
            className="stacked-modal-layer"
            onPointerDown={(event) =>
              event.target === event.currentTarget && close()
            }
          >
            <section role="dialog" aria-label="Owned backdrop dialog">
              Existing dialog
            </section>
          </div>
        </ModalRegistration>
      </ModalProvider>,
    );
    fireEvent.pointerDown(
      document.body.querySelector<HTMLElement>('[data-modal-id="owned-backdrop"]')!,
    );
    expect(close).toHaveBeenCalledOnce();
  });

  it("keeps an explicitly locked application backdrop inert", () => {
    const close = vi.fn();
    render(
      <ModalProvider>
        <ModalRegistration
          id="locked-application-backdrop"
          policy={{ backdrop: false }}
          onClose={close}
        >
          <div className="stacked-modal-layer">
            <section role="alertdialog" aria-label="Locked application dialog">
              Locked dialog
            </section>
          </div>
        </ModalRegistration>
      </ModalProvider>,
    );
    fireEvent.pointerDown(
      document.body.querySelector<HTMLElement>(
        '[data-modal-id="locked-application-backdrop"]',
      )!,
    );
    expect(close).not.toHaveBeenCalled();
  });

  it("restores focus to the control that opened a nested modal", async () => {
    function FocusHarness() {
      const [open, setOpen] = useState(false);
      return (
        <ModalProvider>
          <Button onClick={() => setOpen(true)}>Open focused modal</Button>
          {open && (
            <ModalFrame
              id="focused"
              ariaLabel="Focused"
              title="Focused"
              onClose={() => setOpen(false)}
            >
              <Button onClick={() => setOpen(false)}>Finish focused modal</Button>
            </ModalFrame>
          )}
        </ModalProvider>
      );
    }
    render(<FocusHarness />);
    const opener = screen.getByRole("button", { name: "Open focused modal" });
    opener.focus();
    fireEvent.click(opener);
    const closer = screen.getByRole("button", { name: "Finish focused modal" });
    closer.focus();
    fireEvent.click(closer);
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    expect(opener).toHaveFocus();
  });

  it("updates close policy by stable identifier and reports missing identifiers", () => {
    const close = vi.fn();
    function PolicyUpdater() {
      const stack = useModalStack();
      return <>
        <Button onClick={() => stack.updatePolicy("target", { escape: false })}>
          Lock Escape
        </Button>
        <output aria-label="Missing update">
          {String(stack.updatePolicy("missing", { escape: false }))}
        </output>
      </>;
    }
    render(
      <ModalProvider>
        <PolicyUpdater />
        <ModalFrame id="target" ariaLabel="Target" title="Target" onClose={close}>
          Target content
        </ModalFrame>
      </ModalProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Lock Escape" }));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Missing update")).toHaveTextContent("false");
  });

  it("closes search settings before its owning modal on Escape", () => {
    const close = vi.fn();
    render(
      <ModalProvider>
        <ModalFrame
          id="search-owner"
          ariaLabel="Fixture browser"
          title="Add fixture"
          search={{
            value: "",
            onSearch: () => undefined,
            settingsConfiguration: [{
              kind: "toggle",
              id: "favorites",
              label: "Favorites only",
              value: false,
              offLabel: "All fixtures",
              onLabel: "Favorites",
            }],
          }}
          onClose={close}
        >
          Fixture results
        </ModalFrame>
      </ModalProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Search settings" }));
    expect(screen.getByRole("dialog", { name: "Add fixture search settings" })).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Add fixture search settings" })).not.toBeInTheDocument();
    expect(close).not.toHaveBeenCalled();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  });

  it("stacks search settings and its nested input strictly above the owning modal", () => {
    render(
      <ModalProvider>
        <ModalFrame
          id="search-owner"
          ariaLabel="Fixture browser"
          title="Add fixture"
          search={{
            value: "",
            onSearch: () => undefined,
            settingsInitiallyOpen: true,
            settingsConfiguration: [{
              kind: "text",
              id: "preset",
              label: "Preset name",
              value: "Tour",
              keyboardInitiallyOpen: true,
            }],
          }}
          onClose={() => undefined}
        >
          Fixture results
        </ModalFrame>
      </ModalProvider>,
    );
    const layers = [...document.querySelectorAll<HTMLElement>(".ui-modal-stack-layer")];
    expect(layers).toHaveLength(3);
    expect(layers.map((layer) => Number(layer.style.zIndex)).sort()).toEqual([3000, 3010, 3020]);
    expect(layers.filter((layer) => layer.dataset.modalTop === "true")).toHaveLength(1);
    expect(Number(layers.find((layer) => layer.dataset.modalTop === "true")?.style.zIndex))
      .toBe(3020);
    expect(screen.getByRole("dialog", { name: "Preset name" })).toHaveAttribute("aria-modal", "true");
  });
});
