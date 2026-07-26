import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { HorizontalFaderField } from "../faders";

interface FaderStoryArgs {
  label?: string;
  minimum?: number;
  maximum?: number;
  step?: number;
  value?: number;
  disabled?: boolean;
  accentColor?: string;
}

const meta = {
  title: "Faders/Horizontal fader",
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { source: { type: "dynamic" } },
  },
  args: {
    label: "Environment brightness",
    minimum: 0,
    maximum: 100,
    step: 0.1,
    value: 68,
    disabled: false,
    accentColor: "#176777",
  },
  argTypes: {
    label: { control: "text" },
    minimum: { control: "number" },
    maximum: { control: "number" },
    step: { control: "number" },
    value: { control: { type: "range", min: 0, max: 100, step: 1 } },
    disabled: { control: "boolean" },
    accentColor: { control: "color" },
  },
} satisfies Meta<FaderStoryArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

function FaderExample(args: Required<FaderStoryArgs>) {
  const [value, setValue] = useState(args.value);
  return (
    <div style={{ width: 520 }}>
      <HorizontalFaderField
        label={args.label}
        value={value}
        minimum={args.minimum}
        maximum={args.maximum}
        step={args.step}
        disabled={args.disabled}
        accentColor={args.accentColor}
        display={`${Math.round(value)}%`}
        onChange={setValue}
      />
    </div>
  );
}

export const Default: Story = {
  render: (args) => <FaderExample {...meta.args} {...args} />,
};

export const States: Story = {
  render: () => (
    <div style={{ width: 620, display: "grid", gap: 12 }}>
      <HorizontalFaderField label="Environment brightness" value={68} display="68%" onChange={() => undefined} />
      <HorizontalFaderField label="Pan" value={-45} minimum={-270} maximum={270} step={0.1} display="-45°" accentColor="#378eff" onChange={() => undefined} />
      <HorizontalFaderField label="Disabled level" value={25} disabled display="25%" onChange={() => undefined} />
    </div>
  ),
};
