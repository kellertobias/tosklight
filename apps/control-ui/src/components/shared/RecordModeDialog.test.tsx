import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RecordModeDialog } from "./RecordModeDialog";

afterEach(cleanup);

describe("RecordModeDialog", () => {
  it("retains a textual Record identity and keeps overwrite destructive", () => {
    render(<RecordModeDialog target="Group 3" onChoose={vi.fn()} onCancel={vi.fn()} />);
    const dialog = screen.getByRole("dialog", { name: "Record to Group 3" });
    expect(dialog).toHaveClass("workflow-theme", "record-workflow");
    expect(screen.getByText("RECORD")).toHaveClass("workflow-badge");
    expect(screen.getByRole("button", { name: "Merge" })).toHaveClass("workflow-choice");
    expect(screen.getByRole("button", { name: "Overwrite" })).toHaveClass("danger");
  });

  it.each([
    ["Merge", "merge"],
    ["Overwrite", "overwrite"],
  ] as const)("returns the explicit %s choice", (label, mode) => {
    const choose = vi.fn();
    render(<RecordModeDialog target="Group 3" onChoose={choose} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: label }));
    expect(choose).toHaveBeenCalledWith(mode);
  });

  it("cancels without choosing a recording mode", () => {
    const choose = vi.fn();
    const cancel = vi.fn();
    render(<RecordModeDialog target="Group 3" onChoose={choose} onCancel={cancel} />);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancel).toHaveBeenCalledOnce();
    expect(choose).not.toHaveBeenCalled();
  });
});
