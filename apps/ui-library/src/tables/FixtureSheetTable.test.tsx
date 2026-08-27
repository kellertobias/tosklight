import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FixtureSheetTableView, type FixtureSheetRowView } from "./FixtureSheetTable";

interface Row extends FixtureSheetRowView {
  name: string;
}

const rows: Row[] = [
  { id: "1", fixtureId: "fixture-1", targetKind: "master", parentFixtureId: "fixture-1", childFixtureIds: ["head-1"], indented: false, name: "Mover" },
  { id: "1.1", fixtureId: "head-1", targetKind: "head", parentFixtureId: "fixture-1", childFixtureIds: [], indented: true, name: "Head 1" },
];

describe("FixtureSheetTableView", () => {
  it("preserves row identity, indentation, step state, and keyboard activation", () => {
    const activate = vi.fn();
    const active = vi.fn();
    render(
      <div style={{ height: 300 }}>
        <FixtureSheetTableView
          activeRow={0}
          columns={[
            { id: "id", header: "ID", width: "88px", render: (row) => row.id },
            { id: "name", header: "Name", render: (row) => row.name },
          ]}
          onActivate={activate}
          onActiveRowChange={active}
          presentStep={(row) => ({
            base: row.fixtureId === "fixture-1",
            current: row.fixtureId === "head-1",
            containedBase: false,
            containedCurrent: row.fixtureId === "fixture-1",
          })}
          rows={rows}
          selectedFixtureIds={new Set(["head-1"])}
        />
      </div>,
    );
    const renderedRows = screen.getAllByRole("row");
    expect(renderedRows[1]).toHaveAttribute("data-fixture-kind", "master");
    expect(renderedRows[1]).toHaveClass("fixture-step-contained-current");
    expect(renderedRows[2]).toHaveClass("fixture-head-indented-row", "fixture-step-current", "selected");
    fireEvent.keyDown(renderedRows[2], { key: "Enter" });
    // The sheet is told which modifiers the activation carried, so it can offer a range.
    expect(activate).toHaveBeenCalledWith("head-1", {
      range: false,
      additive: false,
    });
  });
});
