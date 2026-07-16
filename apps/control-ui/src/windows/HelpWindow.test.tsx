import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { HelpCatalogEntry } from "../api/types";
import { HelpMarkdown, HelpNavigation } from "./HelpWindow";

describe("help key rendering", () => {
  it("renders normal and numeric-range keys as keycaps", () => {
    const { container } = render(<HelpMarkdown markdown={"[AT] [+] [0-9] [.] [CLR] [REC]"}/>);
    expect(screen.getByText("AT", { selector: "kbd" })).toBeInTheDocument();
    expect(screen.getByText("+", { selector: "kbd" })).toBeInTheDocument();
    expect(screen.getByText("0-9", { selector: "kbd" })).toBeInTheDocument();
    expect(container.querySelectorAll(".desk-key-number")).toHaveLength(2);
    expect(container.querySelector(".desk-key-clear kbd")).toHaveTextContent("CLR");
    expect(container.querySelector(".desk-key-record kbd")).toHaveTextContent("REC");
    expect(container.querySelector(".desk-key-command kbd")).toHaveTextContent("AT");
  });

  it("labels held and optional keycaps", () => {
    const { container } = render(<HelpMarkdown markdown={"[CLR+] [GRP*]"}/>);
    const held = container.querySelector(".help-key.held") as HTMLElement;
    const optional = container.querySelector(".help-key.optional") as HTMLElement;
    expect(within(held).getByText("CLR", { selector: "kbd" })).toBeInTheDocument();
    expect(within(held).getByText("hold")).toBeInTheDocument();
    expect(within(optional).getByText("GRP", { selector: "kbd" })).toBeInTheDocument();
    expect(within(optional).getByText("optional")).toBeInTheDocument();
  });

  it("visually distinguishes computer keyboard keys", () => {
    const { container } = render(<HelpMarkdown markdown={"[KBD:ENTER] presses [ENT]"}/>);
    const keyboard = container.querySelector(".help-key.keyboard-key") as HTMLElement;
    expect(within(keyboard).getByText("keyboard")).toBeInTheDocument();
    expect(within(keyboard).getByText("ENTER", { selector: "kbd" })).toBeInTheDocument();
    expect(screen.getByText("ENT", { selector: "kbd" })).toBeInTheDocument();
  });
});

describe("help navigation", () => {
  const entries: HelpCatalogEntry[] = [{
    id: "00-quickstart.markdown",
    title: "Quickstart",
    kind: "topic",
    children: [],
  }, {
    id: "01-Show-Setup/index.md",
    title: "Show Setup",
    kind: "folder",
    children: [{
      id: "01-Show-Setup/01-fixtures-patch.md",
      title: "Fixtures & Patch",
      kind: "topic",
      children: [],
    }],
  }];

  it("opens a folder index from its title and expands only from its chevron", () => {
    const onSelect = vi.fn();
    const { rerender } = render(<nav><HelpNavigation entries={entries} expanded={new Set()} selected={null} onSelect={onSelect} onToggle={vi.fn()}/></nav>);

    expect(screen.queryByRole("button", { name: "Expand Quickstart" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show Setup" }));
    expect(onSelect).toHaveBeenCalledWith("01-Show-Setup/index.md");
    expect(screen.queryByRole("button", { name: "Fixtures & Patch" })).not.toBeInTheDocument();

    const onToggle = vi.fn();
    fireEvent.click(screen.getByRole("button", { name: "Expand Show Setup" }));
    rerender(<nav><HelpNavigation entries={entries} expanded={new Set(["01-Show-Setup/index.md"])} selected={null} onSelect={onSelect} onToggle={onToggle}/></nav>);

    expect(screen.getByRole("button", { name: "Collapse Show Setup" })).toHaveAttribute("aria-expanded", "true");
    const child = screen.getByRole("button", { name: "Fixtures & Patch" });
    expect(child.closest(".help-nav-row")).toHaveStyle({ paddingLeft: "28px" });
  });
});
