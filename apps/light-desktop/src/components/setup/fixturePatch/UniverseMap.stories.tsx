import type { Meta, StoryObj } from "@storybook/react-vite";
import { type ComponentProps, useState } from "react";
import type { PatchedFixture } from "../../../api/types";
import "../../../applicationStyles";
import { blankFixtureProfile } from "../fixtureProfileModel";
import { UniverseMap, type UniverseMapProposal } from "./UniverseMap";

const meta = {
	title: "Controls/DMX patch grid",
	component: UniverseMap,
	parameters: { layout: "fullscreen" },
	decorators: [
		(Story) => (
			<div style={{ height: 680, padding: 16 }}>
				<Story />
			</div>
		),
	],
} satisfies Meta<typeof UniverseMap>;

export default meta;
type Story = StoryObj<typeof meta>;

const occupiedFixtures = [
	storyFixture(101, "Front Wash 1", 1, 1, 10),
	storyFixture(102, "Front Wash 2", 1, 21, 10),
	storyFixture(201, "Stage Left Spot", 1, 101, 16),
] satisfies PatchedFixture[];

export const InteractivePlacement: Story = {
	args: {
		fixtures: occupiedFixtures,
		universe: 1,
		proposed: 41,
		footprint: 10,
		proposedLabel: "Fixture 103 · New Wash",
		onAddress: () => undefined,
		onUniverse: () => undefined,
	},
	render: (args) => <InteractivePatchGrid {...args} />,
};

function InteractivePatchGrid(props: ComponentProps<typeof UniverseMap>) {
	const [universe, setUniverse] = useState(props.universe);
	const [proposals, setProposals] = useState<UniverseMapProposal[]>([
		{
			key: "new-fixture",
			start: props.proposed,
			footprint: props.footprint,
			label: props.proposedLabel,
		},
	]);
	return (
		<UniverseMap
			{...props}
			universe={universe}
			proposals={proposals}
			onAddress={(address) =>
				setProposals((current) =>
					current.map((proposal) => ({ ...proposal, start: address })),
				)
			}
			onProposalAddress={(key, address) =>
				setProposals((current) =>
					current.map((proposal) =>
						proposal.key === key ? { ...proposal, start: address } : proposal,
					),
				)
			}
			onUniverse={setUniverse}
		/>
	);
}

function storyFixture(
	fixtureNumber: number,
	name: string,
	universe: number,
	address: number,
	footprint: number,
): PatchedFixture {
	const profile = blankFixtureProfile();
	profile.id = `storybook-profile-${fixtureNumber}`;
	profile.revision = 1;
	profile.manufacturer = "ToskLight Demo";
	profile.name = name;
	profile.short_name = name;
	profile.modes[0].id = `storybook-mode-${fixtureNumber}`;
	profile.modes[0].splits = [{ number: 1, footprint }];
	return {
		fixture_id: `storybook-fixture-${fixtureNumber}`,
		fixture_number: fixtureNumber,
		name,
		definition: {
			schema_version: 2,
			id: profile.id,
			revision: profile.revision,
			manufacturer: profile.manufacturer,
			device_type: "wash",
			name: profile.name,
			model: profile.short_name,
			mode: "Default",
			footprint,
			heads: [],
			color_calibration: null,
			physical: {},
			model_asset: null,
			icon_asset: null,
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
			profile_id: profile.id,
			mode_id: profile.modes[0].id,
			profile_snapshot: profile,
		},
		universe,
		address,
		split_patches: [{ split: 1, universe, address }],
		layer_id: "default",
		direct_control: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
	};
}
