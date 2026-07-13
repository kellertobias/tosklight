import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { HelpMarkdown } from "./HelpWindow";

describe("help key rendering", () => {
  it("renders normal and numeric-range keys as keycaps", () => {
    render(<HelpMarkdown markdown={"[AT] [+] [0-9]"}/>);
    expect(screen.getByText("AT", { selector: "kbd" })).toBeInTheDocument();
    expect(screen.getByText("+", { selector: "kbd" })).toBeInTheDocument();
    expect(screen.getByText("0-9", { selector: "kbd" })).toBeInTheDocument();
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
});
