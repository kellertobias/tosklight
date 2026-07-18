import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SelectionList, SelectionTree } from ".";

afterEach(cleanup);

describe("SelectionList", () => {
  it("uses one measured content wrapper and identical option geometry for empty state", () => {
    const { rerender } = render(<SelectionList ariaLabel="Items" value="one" options={[{ value: "one", label: "One" }]} onChange={() => {}}/>);
    const option = screen.getByRole("radio", { name: "One" });
    expect(option.parentElement).toHaveClass("ui-selection-list");
    rerender(<SelectionList ariaLabel="Items" options={[]} onChange={() => {}}/>);
    const empty = screen.getByRole("status");
    expect(empty).toHaveClass("ui-selection-list-option");
    expect(empty.parentElement).toHaveClass("ui-selection-list");
  });

  it("composes independently selectable columns as a tree", () => {
    const chooseFunction = vi.fn();
    const chooseOption = vi.fn();
    render(<SelectionTree columns={[
      { id: "function", title: "Function", ariaLabel: "Functions", value: "cue", options: [{ value: "cue", label: "Cue List" }], onChange: chooseFunction },
      { id: "option", title: "Options", ariaLabel: "Cue Lists", value: "main", options: [{ value: "main", label: "Main" }], onChange: chooseOption },
    ]}/>);
    expect(screen.getAllByRole("radiogroup")).toHaveLength(2);
    fireEvent.click(screen.getByRole("radio", { name: "Main" }));
    expect(chooseOption).toHaveBeenCalledWith("main");
    expect(chooseFunction).not.toHaveBeenCalled();
  });
});
