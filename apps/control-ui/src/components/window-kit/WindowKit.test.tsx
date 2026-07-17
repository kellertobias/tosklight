import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ButtonGrid, DataTable, GridButton, WindowHeader, WindowScrollArea, WindowSettings } from ".";

describe("window kit", () => {
  it("renders two-line information, grouped actions, and Settings last", () => {
    const { container } = render(<WindowHeader title="Stage" info={{ primary: "1 selected", secondary: <span className="test-legend">Shift for range</span> }} search={<span>Search control</span>} actions={[[{ id: "one", label: "First", onClick: vi.fn() }],[{ id: "two", label: "Second", onClick: vi.fn() }]]} settings onSettings={vi.fn()} />);
    expect(screen.getByText("Stage")).toBeInTheDocument();
    expect(screen.getByText("Shift for range")).toHaveClass("test-legend");
    expect(screen.getByText("Shift for range").parentElement?.tagName).toBe("SMALL");
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual(["First", "Second", "⚙Settings"]);
    const header = container.querySelector(".ui-window-header")!;
    expect([...header.children].indexOf(screen.getByText("Search control").parentElement!)).toBeLessThan([...header.children].indexOf(container.querySelector(".ui-window-action-groups")!));
  });
  it("switches settings tabs and closes", () => {
    const close = vi.fn();
    render(<WindowSettings title="Pane Settings" tabs={[{ id: "pane", label: "Pane Settings", content: "Size" },{ id: "pool", label: "Pool", content: "Family" }]} onClose={close} />);
    fireEvent.click(screen.getByRole("tab", { name: "Pool" }));
    expect(screen.getByText("Family")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close settings" }));
    expect(close).toHaveBeenCalledOnce();
  });
  it("renders built-in settings as an anchored popover without a backdrop", () => {
    render(<WindowSettings modal={false} anchor={new DOMRect(900, 10, 90, 38)} title="Stage Settings" tabs={[{ id: "stage", label: "Stage", content: "Display" }]} onClose={() => undefined} />);
    const dialog = screen.getByRole("dialog", { name: "Stage Settings" });
    expect(dialog).toHaveClass("popover");
    expect(dialog.closest(".ui-window-settings-backdrop")).toBeNull();
  });
  it("keeps selected and active rows independent and navigates empty rows", () => {
    const active = vi.fn();
    render(<DataTable rows={[{ id: "one" },{ id: "two" }]} columns={[{ id: "name", header: "Name", render: (row) => row.id }]} rowKey={(row) => row.id} selected={(row) => row.id === "two"} activeIndex={0} onActiveIndexChange={active} emptyRows={1} />);
    const rows = screen.getAllByRole("row");
    expect(rows[2]).toHaveClass("selected");
    expect(rows[1]).toHaveClass("active");
    fireEvent.keyDown(rows[2], { key: "ArrowDown" });
    expect(active).toHaveBeenCalledWith(2);
  });
  it("exposes button grid states", () => {
    render(<ButtonGrid><GridButton number="1" primary="Open" state="active"/><GridButton number="2" primary="Empty" state="empty"/><GridButton number="3" primary="Disabled" state="disabled"/><GridButton number="4" primary="Store" state="store-target"/></ButtonGrid>);
    expect(screen.getByRole("button", { name: /Open/ })).toHaveClass("active");
    expect(screen.getByRole("button", { name: /Empty/ })).toHaveClass("empty");
    expect(screen.getByRole("button", { name: /Disabled/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Store/ })).toHaveClass("store-target");
  });
  it("uses the untransformed column width for every button-grid row", () => {
    const realGetComputedStyle = window.getComputedStyle.bind(window);
    const computedStyle = vi.spyOn(window, "getComputedStyle").mockImplementation((element) => {
      const style = realGetComputedStyle(element);
      if (element.tagName === "BUTTON") Object.defineProperty(style, "width", { value: "117.25px" });
      return style;
    });
    const rect = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const width = this.tagName === "BUTTON" ? 114.319 : 400;
      return { x: 0, y: 0, top: 0, right: width, bottom: width, left: 0, width, height: width, toJSON: () => ({}) };
    });
    render(<ButtonGrid><GridButton number="1" primary="One"/><GridButton number="2" primary="Two"/></ButtonGrid>);
    expect(screen.getByRole("button", { name: /One/ }).parentElement).toHaveStyle({ "--grid-row-size": "117.25px" });
    computedStyle.mockRestore();
    rect.mockRestore();
  });
  it("shows the unified empty state instead of window content", () => {
    render(<WindowScrollArea emptyState={{ title: "Nothing here", description: "Add an item to get started.", icon: "◇" }}><span>Hidden content</span></WindowScrollArea>);
    expect(screen.getByRole("status")).toHaveTextContent("Nothing here");
    expect(screen.getByText("Add an item to get started.")).toBeInTheDocument();
    expect(screen.queryByText("Hidden content")).not.toBeInTheDocument();
  });
});
