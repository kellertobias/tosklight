import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button, type ButtonVariant } from "../controls";

const meta = {
	title: "ToskLight/Controls/Button",
	component: Button,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj;

const variants: Array<{ variant: ButtonVariant; label: string; icon: string }> =
	[
		{ variant: "primary", label: "Primary", icon: "▶" },
		{ variant: "secondary", label: "Secondary", icon: "✎" },
		{ variant: "ghost", label: "Ghost", icon: "◇" },
		{ variant: "danger", label: "Danger", icon: "⌫" },
		{ variant: "success", label: "Success", icon: "✓" },
		{ variant: "warning", label: "Warning", icon: "⚠" },
	];

export const Primary: Story = {
	render: () => (
		<div className="forms-story-canvas">
			<section>
				<h2>Variants and states</h2>
				<div className="button-catalog-grid">
					{variants.map(({ variant, label }) => (
						<Button key={variant} variant={variant}>
							{label}
						</Button>
					))}
					<Button active>Active</Button>
					<Button disabled>Disabled</Button>
					<Button loading>Save</Button>
					<Button size="compact">Compact</Button>
					<Button iconOnly icon="⚙" aria-label="Settings" />
					<Button fullWidth>Full width</Button>
				</div>
			</section>
			<section>
				<h2>Label and icon content</h2>
				<div className="button-catalog-grid">
					{variants.map(({ variant, label, icon }) => (
						<Button key={variant} variant={variant} icon={icon}>
							{label} with icon
						</Button>
					))}
				</div>
			</section>
			<section>
				<h2>Left-aligned full-width controls</h2>
				<div className="button-catalog-grid">
					{variants.map(({ variant, label }) => (
						<Button
							key={variant}
							variant={variant}
							contentAlign="left"
							fullWidth
						>
							{label} aligned
						</Button>
					))}
				</div>
			</section>
		</div>
	),
};
