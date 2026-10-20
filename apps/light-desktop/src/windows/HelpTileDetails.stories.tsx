/** Focused manual photographs of production tiles, using supported pane layouts and real views. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
	PlaybackTileView,
	type PlaybackCardViewModel,
} from "@tosklight/ui/playback";
import { PoolTileLayoutProvider } from "@tosklight/ui/pools";
import {
	previewColorPresets,
	previewPositionPresets,
} from "../../../ui-library/storybook/fixtures/presetPreviews";
import { SoloFenceOverlay } from "../components/control/playbackFaderBank/SoloFenceOverlay";
import type { Group } from "./groupsWindow/model";
import {
	MarketingGroupsWindow,
	MarketingPresetWindow,
} from "./PoolWindows.stories";
const meta = {
	title: "ToskLight/Windows/Tile details",
	parameters: { layout: "fullscreen" },
	tags: ["autodocs"],
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
function group(
	id: string,
	name: string,
	fixtures: string[],
	extras: Partial<Group["body"]> = {},
): Group {
	return {
		kind: "group",
		id,
		revision: 3,
		updated_at: "2026-07-26T10:00:00Z",
		body: {
			name,
			fixtures,
			programming: {},
			derived_from: null,
			frozen_from: null,
			...extras,
		},
	};
}
const known = new Set(Array.from({ length: 8 }, (_, i) => `fixture-${i + 1}`));
function GroupDetails({ markers = false }: { markers?: boolean }) {
	const cards = markers
		? [
				group("1", "Stage Wash", [
					"fixture-1",
					"fixture-2",
					"fixture-3",
					"fixture-4",
				]),
				group("2", "Stage Wash Odds", ["fixture-1", "fixture-3"], {
					derived_from: { source_group_id: "1", rule: { type: "odd" } },
				}),
				group("3", "Frozen Wash", ["fixture-1", "fixture-2"], {
					frozen_from: {
						source_group_id: "1",
						source_revision: 2,
						captured_at: "2026-07-26T10:00:00Z",
					},
				}),
			]
		: [
				group("1", "Audience Spots", [
					"fixture-5",
					"fixture-6",
					"fixture-7",
					"fixture-8",
				]),
				group("2", "Stage Wash", [
					"fixture-1",
					"fixture-2",
					"fixture-3",
					"fixture-4",
				]),
				group("3", "Front Fresnels", ["fixture-1", "fixture-2"]),
			];
	return (
		<section
			data-help-capture="tile-details"
			style={{ width: 660, height: 252 }}
		>
			<PoolTileLayoutProvider layout={{ mode: "fixed", rows: 1, columns: 3 }}>
				<MarketingGroupsWindow
					cards={cards}
					knownFixtureIds={known}
					width={660}
					height={252}
				/>
			</PoolTileLayoutProvider>
		</section>
	);
}
export const GroupCounts: Story = { render: () => <GroupDetails /> };
export const GroupMarkers: Story = { render: () => <GroupDetails markers /> };
function PresetDetails({ position = false }: { position?: boolean }) {
	const source = position
		? previewPositionPresets.slice(0, 3)
		: [...previewColorPresets.slice(0, 3), previewColorPresets[5]];
	const presets = source.map((preset, i) => ({
		...preset,
		id: `${position ? 3 : 2}.${i + 1}`,
		body: { ...preset.body, number: i + 1 },
	}));
	const counts = new Map(
		presets.map((p, i) => [
			p.id,
			{
				active: i === 0 ? 2 : i === 2 ? 3 : 0,
				defined: 4,
				universal: Boolean(p.body.universal_values),
			},
		]),
	);
	const width = position ? 660 : 880;
	return (
		<section data-help-capture="tile-details" style={{ width, height: 252 }}>
			<PoolTileLayoutProvider
				layout={{ mode: "fixed", rows: 1, columns: presets.length }}
			>
				<MarketingPresetWindow
					family={position ? "Position" : "Color"}
					presets={presets}
					fixtureCounts={counts}
					width={width}
					height={252}
				/>
			</PoolTileLayoutProvider>
		</section>
	);
}
export const ColorPreviewCounts: Story = { render: () => <PresetDetails /> };
export const PositionPreviews: Story = {
	render: () => <PresetDetails position />,
};
const regions = [
	{
		id: "looks",
		name: "Stage looks",
		color: "#22c55e",
		members: [
			{ page: 1, position: 401 },
			{ page: 1, position: 402 },
		],
	},
];
const names = ["Opening Look", "Blue Wash", "Audience Spots"];
const actions = ["GO +", "SOLO · TOGGLE", "TOGGLE"];
function PlaybackDetails({ shallow = false }: { shallow?: boolean }) {
	const [bank, setBank] = useState<HTMLDivElement | null>(null);
	return (
		<section
			data-help-capture="playback-details"
			className="ui-window"
			style={{ width: 756, height: shallow ? 112 : 276, padding: 12 }}
		>
			<div
				ref={setBank}
				style={{
					position: "relative",
					display: "grid",
					gridTemplateColumns: "repeat(3,minmax(0,1fr))",
					gap: 12,
					height: "100%",
				}}
			>
				{names.map((name, i) => {
					const model: PlaybackCardViewModel = {
						page: 1,
						slot: 401 + i,
						row: 1,
						rowUnits: 1,
						name,
						assigned: true,
						kind: "cue-list",
						hasFader: false,
						faderValue: 100,
						faderLabel: "Master",
						faderDisplay: "100%",
						actions: [],
						summary: {
							label: `${i + 1} · ${["Warm Welcome", "Deep Blue", "Audience Sweep"][i]}`,
							detail: "3.0s",
						},
						className: i === 0 ? "running" : "",
						color: ["#d29746", "#285fa8", "#6a4ba8"][i],
					};
					return (
						<PlaybackTileView
							key={name}
							model={model}
							action={{
								id: "primary",
								label: actions[i],
								onClick: () => undefined,
							}}
						/>
					);
				})}
				<SoloFenceOverlay
					bank={bank}
					page={1}
					regions={regions}
					layoutKey={names}
				/>
			</div>
		</section>
	);
}
export const PlaybackCueAndSolo: Story = { render: () => <PlaybackDetails /> };
export const PlaybackShallow: Story = {
	render: () => <PlaybackDetails shallow />,
};
