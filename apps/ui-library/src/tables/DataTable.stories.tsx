import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { DataTable, type DataTableColumn } from "../tables";

interface InventoryRow {
  id: string;
  item: string;
  category: string;
  quantity: number;
  status: "Available" | "Reserved" | "Maintenance";
}

const rows: InventoryRow[] = [
  { id: "asset-001", item: "Wireless microphone", category: "Audio", quantity: 8, status: "Available" },
  { id: "asset-002", item: "Tripod stand", category: "Rigging", quantity: 12, status: "Reserved" },
  { id: "asset-003", item: "Power distribution unit", category: "Power", quantity: 4, status: "Available" },
  { id: "asset-004", item: "Network switch", category: "Data", quantity: 2, status: "Maintenance" },
];

const columns: DataTableColumn<InventoryRow>[] = [
  { id: "id", header: "Asset ID", width: "130px", render: (row) => row.id },
  { id: "item", header: "Item", width: "minmax(220px, 1fr)", render: (row) => row.item },
  { id: "category", header: "Category", width: "140px", render: (row) => row.category },
  { id: "quantity", header: "Quantity", width: "100px", align: "right", render: (row) => row.quantity },
  { id: "status", header: "Status", width: "150px", render: (row) => row.status },
];

interface GenericTableArgs {
  emptyRows: number;
  selectedId: string;
}

const meta = {
  title: "Tables and Grids/Generic table",
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { source: { type: "dynamic" } },
  },
  args: {
    emptyRows: 3,
    selectedId: "asset-002",
  },
  argTypes: {
    emptyRows: { control: { type: "range", min: 0, max: 8, step: 1 } },
    selectedId: {
      control: "select",
      options: rows.map((row) => row.id),
    },
  },
} satisfies Meta<GenericTableArgs>;

export default meta;
type Story = StoryObj<GenericTableArgs>;

function InteractiveTable({ emptyRows, selectedId: initialSelectedId }: GenericTableArgs) {
  const [activeIndex, setActiveIndex] = useState(0);
  const [selectedId, setSelectedId] = useState(initialSelectedId);
  const [lastActivated, setLastActivated] = useState("None");

  return (
    <div style={{ display: "grid", gap: 12, height: 420, padding: 16 }}>
      <DataTable
        columns={columns}
        rows={rows}
        rowKey={(row) => row.id}
        selected={(row) => row.id === selectedId}
        rowDataAttributes={(row) => ({ "data-asset-id": row.id })}
        activeIndex={activeIndex}
        onActiveIndexChange={setActiveIndex}
        onActivate={(row) => {
          setSelectedId(row.id);
          setLastActivated(`${row.id} · ${row.item}`);
        }}
        emptyRows={emptyRows}
      />
      <output aria-label="Generic table state">
        Active row: {activeIndex + 1} · Selected: {selectedId} · Last activated: {lastActivated}
      </output>
    </div>
  );
}

export const Interactive: Story = {
  render: (args) => <InteractiveTable key={args.selectedId} {...args} />,
};

export const Empty: Story = {
  args: {
    emptyRows: 6,
  },
  render: (args) => (
    <div style={{ height: 360, padding: 16 }}>
      <DataTable
        columns={columns}
        rows={[]}
        rowKey={(row) => row.id}
        emptyRows={args.emptyRows}
      />
    </div>
  ),
};
