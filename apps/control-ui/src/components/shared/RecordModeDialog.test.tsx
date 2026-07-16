import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RecordModeDialog } from "./RecordModeDialog";

afterEach(cleanup);

describe("RecordModeDialog", () => {
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
