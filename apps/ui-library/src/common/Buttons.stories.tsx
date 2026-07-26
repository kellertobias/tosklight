import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, type ButtonVariant } from "../controls";

const meta = {
  title: "Controls/Buttons",
  component: Button,
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { source: { type: "dynamic" } },
  },
  argTypes: {
    variant: {
      control: "select",
      options: ["primary", "secondary", "ghost", "danger", "success", "warning"],
    },
    size: { control: "inline-radio", options: ["default", "compact"] },
    active: { control: "boolean" },
    loading: { control: "boolean" },
    disabled: { control: "boolean" },
    fullWidth: { control: "boolean" },
    iconOnly: { control: "boolean" },
    icon: { control: "text" },
    contentAlign: {
      control: "inline-radio",
      options: ["center", "left"],
    },
  },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ButtonPlayground: Story = {
  args: {
    children: "Apply",
    variant: "secondary",
    size: "default",
    active: false,
    loading: false,
    disabled: false,
    fullWidth: false,
    iconOnly: false,
    contentAlign: "center",
  },
  render: (args) => <Button {...args} />,
};

export const Buttons: Story = {
  render: () => (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(4, minmax(0, 1fr))", gap: 10 }}>
      <Button variant="primary">Primary</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="ghost">Ghost</Button>
      <Button variant="danger">Danger</Button>
      <Button variant="success">Success</Button>
      <Button variant="warning">Warning</Button>
      <Button active>Active</Button>
      <Button disabled>Disabled</Button>
      <Button loading>Save</Button>
      <Button size="compact">Compact</Button>
      <Button iconOnly icon="⚙" aria-label="Settings" />
      <Button fullWidth>Full width</Button>
    </div>
  ),
};

const buttonVariantExamples: Array<{
  variant: ButtonVariant;
  label: string;
  icon: string;
}> = [
  { variant: "primary", label: "Primary", icon: "▶" },
  { variant: "secondary", label: "Secondary", icon: "✎" },
  { variant: "ghost", label: "Ghost", icon: "◇" },
  { variant: "danger", label: "Danger", icon: "⌫" },
  { variant: "success", label: "Success", icon: "✓" },
  { variant: "warning", label: "Warning", icon: "⚠" },
];

export const ButtonsWithIcons: Story = {
  render: () => (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 10 }}>
      {buttonVariantExamples.map(({ variant, label, icon }) => (
        <Button key={variant} variant={variant} icon={icon}>
          {label}
        </Button>
      ))}
    </div>
  ),
};

export const LeftAlignedButtons: Story = {
  render: () => (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(220px, 1fr))", gap: 10 }}>
      {buttonVariantExamples.map(({ variant, label }) => (
        <Button key={variant} variant={variant} contentAlign="left" fullWidth>
          {label}
        </Button>
      ))}
    </div>
  ),
};
