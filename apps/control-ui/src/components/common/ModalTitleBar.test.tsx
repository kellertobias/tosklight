import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ModalTitleBar } from "./ModalTitleBar";

describe("ModalTitleBar", () => {
  it("renders a continuous title and close control", () => {
    const close = vi.fn();
    render(<ModalTitleBar title="Number input" onClose={close}/>);
    expect(screen.getByRole("heading", { name: "Number input" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close modal" }));
    expect(close).toHaveBeenCalledOnce();
  });

  it("supports title tabs and right-side actions", () => {
    const select = vi.fn();
    render(<ModalTitleBar title="Settings" tabs={[{ id: "general", label: "General" }, { id: "output", label: "Output" }]} activeTab="general" onTabChange={select} actions={<span>Reset</span>}/>);
    expect(screen.getByRole("tab", { name: "General" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: "Output" }));
    expect(select).toHaveBeenCalledWith("output");
    expect(screen.getByText("Reset")).toBeInTheDocument();
  });
});
