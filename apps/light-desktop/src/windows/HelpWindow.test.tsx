import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { HelpCatalogEntry } from "../api/types";
import {
  filterHelpEntries,
  HelpMarkdown,
  HelpNavigation,
  HelpWindow,
  HelpWindowView,
} from "./HelpWindow";

const helpClient = vi.hoisted(() => ({
  helpCatalog: vi.fn(),
  helpTopic: vi.fn(),
}));
vi.mock("../api/client/api", () => ({
  createLightApi: () => ({ help: helpClient }),
}));

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.clearAllMocks();
});

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

  it("filters nested topics while retaining their real folder hierarchy", () => {
    expect(filterHelpEntries(entries, "fixtures")).toEqual([{
      ...entries[1],
      children: [entries[1].children[0]],
    }]);
    expect(filterHelpEntries(entries, "show setup")).toEqual([entries[1]]);
    expect(filterHelpEntries(entries, "missing")).toEqual([]);
  });

  it("uses typed window search and preserves a clear empty-result state", () => {
    function SearchableHelp() {
      const [query, setQuery] = useState("");
      return <HelpWindowView
        catalog={{ topics: entries, errors: [], live: true }}
        onQueryChange={setQuery}
        onSelect={vi.fn()}
        query={query}
        selected={null}
        topic={null}
      />;
    }
    render(<SearchableHelp />);
    expect(screen.getByText("Live documentation")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Search Help" }), { target: { value: "Fixtures" } });
    expect(screen.queryByRole("button", { name: "Quickstart" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show Setup" })).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Search Help" }), { target: { value: "No such topic" } });
    expect(screen.getByText("No matching help topics.")).toBeInTheDocument();
  });
});

describe("live Help adapter", () => {
  it("loads the catalog and selected topic, then refreshes live documentation", async () => {
    vi.useFakeTimers();
    const catalog = {
      topics: [{ id: "00-quickstart.md", title: "Quickstart", kind: "topic" as const, children: [] }],
      errors: [],
      live: true,
    };
    helpClient.helpCatalog.mockResolvedValue(catalog);
    helpClient.helpTopic.mockResolvedValue({
      id: "00-quickstart.md",
      title: "Quickstart",
      markdown: "# Quickstart",
      live: true,
    });

    render(<HelpWindow />);
    await act(async () => undefined);
    await act(async () => undefined);
    expect(helpClient.helpCatalog).toHaveBeenCalledOnce();
    expect(helpClient.helpTopic).toHaveBeenCalledWith("00-quickstart.md");
    expect(screen.getByRole("heading", { name: "Quickstart" })).toBeInTheDocument();

    await act(() => vi.advanceTimersByTimeAsync(1_000));
    await act(async () => undefined);
    expect(helpClient.helpCatalog).toHaveBeenCalledTimes(2);
    expect(helpClient.helpTopic).toHaveBeenCalledTimes(2);
  });
});
