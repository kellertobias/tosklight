import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { FixtureSheetTableView, type FixtureSheetRowView } from "../tables";

interface Row extends FixtureSheetRowView {
  name: string;
  patch: string;
}

const rows: Row[] = [
  { id: "101", fixtureId: "fixture-101", targetKind: "master", parentFixtureId: "fixture-101", childFixtureIds: ["fixture-101-head-1"], indented: false, name: "Stage Left Mover", patch: "U1.101" },
  { id: "101.1", fixtureId: "fixture-101-head-1", targetKind: "head", parentFixtureId: "fixture-101", childFixtureIds: [], indented: true, name: "Head 1", patch: "U1.101" },
  { id: "102", fixtureId: "fixture-102", targetKind: "fixture", parentFixtureId: "fixture-102", childFixtureIds: [], indented: false, name: "Front Wash", patch: "Unpatched" },
];

const meta = {
  title: "ToskLight/Tables/Fixture Sheet Table",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function FixtureSheetExample() {
  const [active, setActive] = useState(1);
  return (
    <div style={{ height: 520 }}>
      <FixtureSheetTableView
        activeRow={active}
        columns={[
          { id: "id", header: "ID", width: "88px", render: (row) => row.id },
          { id: "name", header: "Name / type", width: "minmax(190px, 1.4fr)", render: (row) => row.name },
          { id: "patch", header: "Patch", width: "130px", render: (row) => row.patch },
        ]}
        rows={rows}
        selectedFixtureIds={new Set(["fixture-101-head-1"])}
        onActiveRowChange={setActive}
        onActivate={() => undefined}
        presentStep={(row) => ({
          base: row.fixtureId === "fixture-101",
          containedBase: false,
          current: row.fixtureId === "fixture-101-head-1",
          containedCurrent: row.fixtureId === "fixture-101",
        })}
      />
    </div>
  );
}

export const StepSelection: Story = {
  render: () => <FixtureSheetExample />,
};
