import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../controls";
import {
  ButtonGrid,
  DataTable,
  GridButton,
  SelectionList,
  SelectionTree,
  WindowFrame,
  WindowHeader,
  WindowScrollArea,
  WindowSettings,
} from "../window-kit";

interface WindowStoryArgs {
  title?: string;
  primaryInfo?: string;
  secondaryInfo?: string;
  showNavigation?: boolean;
  showInformation?: boolean;
  showSettings?: boolean;
  showBottom?: boolean;
}

const meta = {
  title: "Windows/Production window kit",
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { source: { type: "dynamic" } },
  },
  args: {
    title: "Fixture Sheet",
    primaryInfo: "12 fixtures",
    secondaryInfo: "4 selected",
    showNavigation: true,
    showInformation: true,
    showSettings: true,
    showBottom: true,
  },
  argTypes: {
    title: { control: "text" },
    primaryInfo: { control: "text" },
    secondaryInfo: { control: "text" },
    showNavigation: { control: "boolean" },
    showInformation: { control: "boolean" },
    showSettings: { control: "boolean" },
    showBottom: { control: "boolean" },
  },
} satisfies Meta<WindowStoryArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Configuration: Story = {
  render: (storyArgs) => {
    const args = { ...meta.args, ...storyArgs };
    const [search, setSearch] = useState("");
    return (
    <div style={{ height: 620, containerType: "inline-size" }}>
      <WindowFrame
        title={args.title}
        info={{ primary: args.primaryInfo, secondary: args.secondaryInfo }}
        search={{ value: search }}
        onSearch={setSearch}
        actions={[
          [{ id: "highlight", label: "Highlight", active: true, onClick: () => undefined }],
          [{ id: "select", label: "Select fixtures", onClick: () => undefined }],
        ]}
        settingsTabs={args.showSettings ? [
          { id: "columns", label: "Columns", content: "Column settings" },
          { id: "display", label: "Display", content: "Display settings" },
        ] : []}
        navigation={args.showNavigation ? <Button fullWidth>All fixtures</Button> : undefined}
        infoSection={args.showInformation ? <p>Fixture and logical-head information</p> : undefined}
        bottom={args.showBottom ? <div style={{ padding: 10 }}>Programmer values use LTP semantics.</div> : undefined}
      >
        <WindowScrollArea>
          <div style={{ padding: 12 }}>Production window composition</div>
        </WindowScrollArea>
      </WindowFrame>
    </div>
    );
  },
};

export const HeaderConfigurations: Story = {
  render: () => (
    <div style={{ display: "grid", gap: 12 }}>
      <WindowHeader title="Stage" />
      <WindowHeader
        title="Fixture Sheet"
        info={{ primary: "1 selected", secondary: "Shift for range" }}
        search={{ value: "" }}
        onSearch={() => undefined}
        toolbar={<Button size="compact">Custom control</Button>}
        actions={[
          [{ id: "follow", label: "Follow Preload", active: true, onClick: () => undefined }],
          [{ id: "select", label: "Select fixtures", onClick: () => undefined }],
        ]}
        settings
        onSettings={() => undefined}
      />
      <WindowHeader
        title="Cuelist"
        onTitleClick={() => undefined}
        titleActionLabel="Remove Cuelist pane"
        actions={[[{
          id: "store",
          label: "Store",
          ariaLabel: "Store cue",
          onClick: () => undefined,
          onLongPress: () => undefined,
        }]]}
      />
    </div>
  ),
};

export const SettingsConfigurations: Story = {
  render: () => (
    <div style={{ minHeight: 420 }}>
      <WindowSettings
        title="Pane Settings"
        tabs={[
          { id: "pane", label: "Pane Settings", content: <p>Size and placement</p> },
          { id: "pool", label: "Pool", content: <p>Family and labels</p> },
        ]}
        onClose={() => undefined}
      />
    </div>
  ),
};

export const AnchoredSettings: Story = {
  render: () => (
    <div style={{ minHeight: 360 }}>
      <WindowSettings
        modal={false}
        anchor={new DOMRect(80, 40, 120, 38)}
        title="Stage Settings"
        tabs={[{ id: "stage", label: "Stage", content: <p>Display</p> }]}
        onClose={() => undefined}
      />
    </div>
  ),
};

export const ScrollAndEmptyStates: Story = {
  render: () => (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, height: 360 }}>
      <WindowScrollArea>
        <div style={{ display: "grid", gap: 8, padding: 8 }}>
          {Array.from({ length: 18 }, (_, index) => <Button key={index}>Item {index + 1}</Button>)}
        </div>
      </WindowScrollArea>
      <WindowScrollArea emptyState={{
        title: "Nothing here",
        description: "Add an item to get started.",
        icon: "◇",
        action: <Button variant="primary">Add item</Button>,
      }} />
    </div>
  ),
};

interface TableRow {
  id: string;
  name: string;
  type: string;
}

function TableExample() {
  const [active, setActive] = useState(0);
  const rows: TableRow[] = [
    { id: "1", name: "Front Wash", type: "Fixture" },
    { id: "1.1", name: "Head 1", type: "Logical head" },
    { id: "2", name: "Back Spot", type: "Fixture" },
  ];
  return (
    <div style={{ height: 360 }}>
      <DataTable
        rows={rows}
        columns={[
          { id: "id", header: "ID", width: "90px", render: (row) => row.id },
          { id: "name", header: "Name", width: "minmax(180px, 1fr)", render: (row) => row.name },
          { id: "type", header: "Type", width: "160px", render: (row) => row.type },
        ]}
        rowKey={(row) => row.id}
        selected={(row) => row.id === "2"}
        rowClassName={(row) => row.type === "Logical head" ? "fixture-head-row" : ""}
        rowDataAttributes={(row) => ({ "data-fixture-id": row.id })}
        activeIndex={active}
        onActiveIndexChange={setActive}
        onActivate={() => undefined}
        emptyRows={3}
      />
    </div>
  );
}

export const Table: Story = {
  render: () => <TableExample />,
};

export const PoolGrid: Story = {
  render: () => (
    <div style={{ width: 560 }}>
      <ButtonGrid>
        <GridButton number="1" primary="All" secondary="12 fixtures" state="active" />
        <GridButton number="2" primary="Front Wash" secondary="4 fixtures" />
        <GridButton number="2.1" primary="Selected" secondary="Logical heads" state="selected" />
        <GridButton number="3" primary="Empty" state="empty" />
        <GridButton number="4" primary="Disabled" state="disabled" />
        <GridButton number="5" primary="Store here" state="store-target" />
      </ButtonGrid>
    </div>
  ),
};

export const MultiStepSelection: Story = {
  render: () => <MultiStepExample />,
};

function MultiStepExample() {
  const [selectedFunction, setSelectedFunction] = useState("cue");
  const [selectedOption, setSelectedOption] = useState("main");
  return (
    <div style={{ height: 440 }}>
      <SelectionTree columns={[
        {
          id: "function",
          title: "Function",
          ariaLabel: "Functions",
          value: selectedFunction,
          options: [
            { value: "cue", label: "Cue List", description: "Playback source" },
            { value: "speed", label: "Speed Group", description: "Timing source" },
          ],
          onChange: setSelectedFunction,
        },
        {
          id: "option",
          title: "Options",
          ariaLabel: "Cue Lists",
          value: selectedOption,
          options: [
            { value: "main", label: "Main", description: "6 cues" },
            { value: "encore", label: "Encore", description: "3 cues" },
          ],
          onChange: setSelectedOption,
          footer: <Button fullWidth>Clear assignment</Button>,
        },
        {
          id: "detail",
          title: "Presentation",
          ariaLabel: "Presentation options",
          value: "touch",
          options: [
            { value: "touch", label: "Touch expanded" },
            { value: "hardware", label: "Hardware reduced", disabled: true },
          ],
          onChange: () => undefined,
        },
      ]} />
    </div>
  );
}

export const SelectionListStates: Story = {
  render: () => (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, height: 300 }}>
      <SelectionList
        ariaLabel="Available windows"
        value="stage"
        options={[
          { value: "stage", label: "Stage", description: "2D and 3D fixture view" },
          { value: "fixture-sheet", label: "Fixture Sheet", description: "Fixture values" },
          { value: "delete", label: "Delete", description: "Remove this assignment", tone: "danger" },
          { value: "unavailable", label: "Unavailable", disabled: true },
        ]}
        onChange={() => undefined}
      />
      <SelectionList
        ariaLabel="Empty options"
        options={[]}
        emptyLabel="No options are available"
        onChange={() => undefined}
      />
    </div>
  ),
};
