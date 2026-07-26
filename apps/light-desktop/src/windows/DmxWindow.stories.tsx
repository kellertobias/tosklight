import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
  dmxOutputHealth,
  dmxPatchedFixtures,
  dmxSnapshot,
  dmxSnapshotWithoutOverrides,
} from "../../../ui-library/storybook/fixtures/dmx";
import type { DmxSnapshot } from "../api/types";
import { DmxWindowView, type DmxWindowViewProps } from "./DmxWindow";

const meta = {
  title: "Application/Windows/DMX",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function DmxStoryHarness(props: Partial<DmxWindowViewProps>) {
  const [dotSize, setDotSize] = useState<"small" | "large">(props.dotSize ?? "small");
  const [snapshot, setSnapshot] = useState<DmxSnapshot | null>(props.snapshot ?? dmxSnapshot);
  const [lastMutation, setLastMutation] = useState("none");
  return <div style={{ height: props.compact ? 500 : 761, minWidth: 0 }}>
    <DmxWindowView
      compact={props.compact}
      defaultSelection={props.defaultSelection}
      defaultView={props.defaultView}
      dotSize={dotSize}
      onDotSizeChange={setDotSize}
      onSetDmxOverride={(universe, address, value) => {
        setLastMutation(`${universe}.${address}:${value ?? "released"}`);
        setSnapshot((current) => current && ({
          ...current,
          overrides: value == null
            ? current.overrides.filter((item) => item.universe !== universe || item.address !== address)
            : [
              ...current.overrides.filter((item) => item.universe !== universe || item.address !== address),
              { universe, address, value },
            ],
        }));
      }}
      outputHealth={props.outputHealth ?? dmxOutputHealth}
      outputRoutes={props.outputRoutes ?? []}
      patchedFixtures={props.patchedFixtures ?? dmxPatchedFixtures}
      snapshot={snapshot}
    />
    <output aria-label="Last DMX mutation" hidden>{lastMutation}</output>
  </div>;
}

export const ValuesOutputSummary: Story = {
  render: () => <DmxStoryHarness snapshot={dmxSnapshotWithoutOverrides} />,
};

export const SelectedPatchedChannel: Story = {
  render: () => <DmxStoryHarness defaultSelection={{ universe: 1, address: 13 }} />,
};

export const SourcesWithOverrides: Story = {
  render: () => <DmxStoryHarness defaultView="sources" />,
};

export const SourcesEmpty: Story = {
  render: () => <DmxStoryHarness defaultView="sources" snapshot={dmxSnapshotWithoutOverrides} />,
};

export const SmallDots: Story = {
  render: () => <DmxStoryHarness dotSize="small" />,
};

export const LargeDots: Story = {
  render: () => <DmxStoryHarness dotSize="large" />,
};

export const Compact: Story = {
  render: () => <DmxStoryHarness compact defaultSelection={{ universe: 1, address: 13 }} />,
};
