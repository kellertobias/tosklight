import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
  GridDesktop,
  PaneView,
  gridRectsOverlap,
  type GridRect,
  type PaneViewModel,
} from "../desktop";

const initialPanes: PaneViewModel[] = [
  { id: "stage", title: "Stage", type: "stage", x: 1, y: 1, width: 12, height: 10 },
  { id: "fixtures", title: "Fixture Sheet", type: "fixtures", x: 13, y: 1, width: 12, height: 10 },
  { id: "groups", title: "Groups", type: "groups", x: 1, y: 11, width: 8, height: 8 },
];

const meta = {
  title: "Window System/Desktop/Grid manager",
  component: GridDesktop,
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
  args: {
    id: "storybook",
    name: "Main",
    editing: false,
  },
  argTypes: {
    id: { control: "text" },
    name: { control: "text" },
    editing: { control: "boolean" },
    empty: { control: "boolean" },
  },
} satisfies Meta<typeof GridDesktop>;

export default meta;
type Story = StoryObj<typeof meta>;

function PaneContents({ title }: { title: string }) {
  return (
    <div style={{ height: "100%", display: "grid", placeItems: "center", color: "var(--muted)" }}>
      {title} content
    </div>
  );
}

function DesktopExample({ editing = false }: { editing?: boolean }) {
  const [panes, setPanes] = useState<PaneViewModel[]>(initialPanes);
  const update = (id: string, rect: GridRect) => {
    setPanes((current) => current.map((pane) => pane.id === id ? { ...pane, ...rect } : pane));
  };
  return (
    <div style={{ width: 1496, height: 761 }}>
      <GridDesktop id="storybook" name="Main" editing={editing}>
        {panes.map((pane) => (
          <PaneView
            key={pane.id}
            pane={pane}
            editing={editing}
            info={{ primary: pane.id === "fixtures" ? "12 fixtures" : "Ready" }}
            settings
            onSettings={() => undefined}
            acceptRect={(candidate) => !panes.some((other) => other.id !== pane.id && gridRectsOverlap(candidate, other))}
            onRectChange={(rect) => update(pane.id, rect)}
          >
            <PaneContents title={pane.title} />
          </PaneView>
        ))}
      </GridDesktop>
    </div>
  );
}

export const ConstrainedPlacement: Story = {
  render: (args) => <DesktopExample editing={args.editing} />,
};

export const DragAndResize: Story = {
  args: { editing: true },
  render: () => <DesktopExample editing />,
};

export const Editing: Story = {
  args: { editing: true },
  render: () => <DesktopExample editing />,
};

export const Maximized: Story = {
  render: () => (
    <div style={{ width: 1496, height: 761 }}>
      <GridDesktop id="maximized" name="Main">
        <PaneView
          pane={{ id: "fixtures", title: "Fixture Sheet", type: "fixtures", x: 1, y: 1, width: 24, height: 18 }}
          maximized
          info={{ primary: "12 fixtures", secondary: "Maximized" }}
          settings
          onSettings={() => undefined}
        >
          <PaneContents title="Fixture Sheet" />
        </PaneView>
      </GridDesktop>
    </div>
  ),
};

function EmptyGridExample() {
  const [opened, setOpened] = useState<GridRect | null>(null);
  return (
    <div style={{ width: 1496, height: 761 }}>
      <GridDesktop id="empty" name="Empty" empty onOpen={setOpened} />
      <output
        aria-label="Requested grid rectangle"
        style={{ position: "absolute", inset: "auto 32px 32px auto", color: "var(--cyan)" }}
      >
        {opened ? `${opened.x},${opened.y} ${opened.width}×${opened.height}` : "No window requested"}
      </output>
    </div>
  );
}

export const EmptyGrid: Story = {
  render: () => <EmptyGridExample />,
};
