import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
  FaderView,
  TouchValueButton,
  VerticalTouchFaderSurface,
} from "../faders";

const meta = {
  title: "Controls/Faders/Vertical touch fader",
  component: VerticalTouchFaderSurface,
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { source: { type: "dynamic" } },
  },
  args: {
    label: "Intensity",
    value: 62.8,
    display: "62.8%",
    directInput: true,
    hardware: false,
    disabled: false,
    accentColor: "#176777",
    actions: [{ id: "flash", label: "FLASH" }],
    onChange: () => undefined,
  },
  argTypes: {
    label: { control: "text" },
    value: { control: { type: "range", min: 0, max: 100, step: 0.1 } },
    display: { control: "text" },
    directInput: { control: "boolean" },
    hardware: { control: "boolean" },
    disabled: { control: "boolean" },
    accentColor: { control: "color" },
  },
} satisfies Meta<typeof VerticalTouchFaderSurface>;

export default meta;
type Story = StoryObj<typeof meta>;

function Surface({
  hardware,
  initialValue,
  label,
  disabled,
  accentColor,
}: {
  hardware: boolean;
  initialValue: number;
  label: string;
  disabled: boolean;
  accentColor?: string;
}) {
  const [value, setValue] = useState(initialValue);
  return (
    <div className={hardware ? "hardware-connected" : ""} style={{ width: 150, height: 520 }}>
      <VerticalTouchFaderSurface
        label={label}
        value={value}
        display={`${value.toFixed(1)}%`}
        directInput
        hardware={hardware}
        disabled={disabled}
        accentColor={accentColor}
        actions={[{ id: "flash", label: "FLASH" }]}
        onChange={setValue}
      />
    </div>
  );
}

export const Software: Story = {
  render: (args) => (
    <Surface
      hardware={Boolean(args.hardware)}
      initialValue={args.value ?? 62.8}
      label={args.label ?? "Intensity"}
      disabled={Boolean(args.disabled)}
      accentColor={args.accentColor}
    />
  ),
};

export const HardwareReduced: Story = {
  args: { hardware: true },
  render: (args) => (
    <Surface
      hardware
      initialValue={args.value ?? 62.8}
      label={args.label ?? "Intensity"}
      disabled={Boolean(args.disabled)}
      accentColor={args.accentColor}
    />
  ),
};

function ValueButtonExample() {
  const [value, setValue] = useState(42);
  return (
    <div style={{ width: 240 }}>
      <TouchValueButton
        label="Grand Master"
        value={value}
        display={`${value.toFixed(1)}%`}
        onChange={setValue}
      />
    </div>
  );
}

export const DirectValueButton: Story = {
  render: () => <ValueButtonExample />,
};

export const FaderViewComposition: Story = {
  render: () => (
    <div style={{ width: 420, height: 520 }}>
      <FaderView rows={1}>
        <Surface hardware={false} initialValue={62.8} label="Intensity" disabled={false} />
        <Surface hardware={false} initialValue={35} label="Speed" disabled={false} accentColor="#378eff" />
      </FaderView>
    </div>
  ),
};
