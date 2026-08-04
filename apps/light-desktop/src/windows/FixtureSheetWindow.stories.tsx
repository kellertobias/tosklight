import type { Meta, StoryObj } from "@storybook/react-vite";
import { FixtureSheetTableView as FixtureSheetTable } from "@tosklight/ui/tables";
import { useMemo, useState } from "react";
import { fixtureTypeIconAsset } from "../components/setup/fixtureTypeIconAssets";
import { SourceLegend } from "../components/shared/SourceLegend";
import type { FixtureSheetCompactMode } from "../types";
import { DEFAULT_FIXTURE_SHEET_COLUMNS } from "./FixtureSheetSettings";
import { FixtureSheetWindowView } from "./FixtureSheetWindow";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
import type { FixtureSheetRow } from "./fixtureSheetProjection";
import type { FixtureStepPresenter } from "./fixtureSheetStep";
import type {
	FixtureSheetDynamicIdentity,
	FixtureSheetGroupValues,
	FixtureSheetMemberValue,
} from "./fixtureSheetValues";

const profileIcon = fixtureTypeIconAsset("profile dimmer lamp");
const washIcon = fixtureTypeIconAsset("led wash moving light");

const runningDynamic = (
	label: string,
	attribute: string,
	overrides: Partial<FixtureSheetDynamicIdentity> = {},
): FixtureSheetDynamicIdentity => ({
	lane: "normal",
	attribute,
	label,
	accessibleName: `Dynamic ${label}, running, winning`,
	poolNumber: Number(label),
	dynamicId: `dynamic-${label}`,
	paused: false,
	pending: false,
	hidden: false,
	winning: true,
	...overrides,
});

const member = (
	attribute: string,
	label: string,
	text: string,
	value: FixtureSheetMemberValue["value"],
	dynamics: FixtureSheetDynamicIdentity[] = [],
	preloadText: string | null = null,
): FixtureSheetMemberValue => ({
	attribute,
	label,
	text,
	value,
	preloadValue: preloadText == null ? null : { kind: "normalized", value: 0.8 },
	preloadText,
	source: "programmer",
	dynamics,
});

const storyGroupValues = (() => {
	const groups = {
		intensity: [
			member(
				"intensity",
				"Intensity",
				"72%",
				{ kind: "normalized", value: 0.72 },
				[
					runningDynamic("7", "intensity"),
					runningDynamic("12", "intensity", {
						accessibleName: "Dynamic 12, paused, hidden, non-winning",
						paused: true,
						hidden: true,
						winning: false,
					}),
				],
				"80%",
			),
		],
		color: [
			member("color.red", "Red", "RGB 246, 201, 133", {
				kind: "normalized",
				value: 0.96,
			}),
		],
		position: [
			member("pan", "Pan", "42°", { kind: "normalized", value: 0.42 }),
			member("tilt", "Tilt", "38°", { kind: "normalized", value: 0.38 }),
		],
		beam: [member("gobo", "Gobo", "Open", { kind: "discrete", value: "open" })],
		shapers: [
			member("shaper.blade.1.position", "Blade 1", "25%", {
				kind: "normalized",
				value: 0.25,
			}),
		],
		focus: [
			member("focus", "Focus", "Sharp", {
				kind: "discrete",
				value: "sharp",
			}),
		],
		control: [
			member("control.mode", "Fixture Mode", "Standard", {
				kind: "discrete",
				value: "standard",
			}),
		],
		media: [
			member("media.folder", "Media Folder", "Folder 2", {
				kind: "discrete",
				value: "2",
			}),
			member("media.file", "Media File", "File 7", {
				kind: "discrete",
				value: "7",
			}),
			member("media.mask.folder", "Mask Folder", "Mask Folder 1", {
				kind: "discrete",
				value: "1",
			}),
			member("media.mask.file", "Mask File", "Mask File 4", {
				kind: "discrete",
				value: "4",
			}),
		],
	};
	return Object.fromEntries(
		Object.entries(groups).map(([id, members]) => [
			id,
			{
				id,
				members,
				available: true,
				source: "programmer",
				accessibleName: members
					.map((value) => `${value.label}: ${value.text}`)
					.join("; "),
			},
		]),
	) as FixtureSheetGroupValues;
})();

const storyLimitedGroup = {
	kind: "group" as const,
	id: "front",
	revision: 1,
	updated_at: "2026-08-02T10:00:00Z",
	body: { name: "Front", fixtures: ["fixture-101"], programming: {} },
	runtime: { master: 0.4, flashLevel: 0, playbackNumber: 17 },
} as FixtureSheetRow["limitingGroups"][number];
const storyFlashedGroup = {
	...storyLimitedGroup,
	id: "movers",
	body: { ...storyLimitedGroup.body, name: "Movers" },
	runtime: { master: 0.4, flashLevel: 0.8, playbackNumber: 18 },
};

const common = {
	beam: "Open",
	childFixtureIds: [] as string[],
	color: "#ffffff",
	colorLabel: "Open White",
	dimmer: 0,
	focus: "Sharp",
	indented: false,
	limitingGroups: [storyLimitedGroup],
	highlightBypassesGroupMaster: false,
	groupValues: storyGroupValues,
	parentFixtureId: "",
	pan: 50,
	patch: "U1.1",
	positionLabel: "Center",
	preloadColor: null,
	preloadDimmer: null,
	preloadPan: null,
	preloadTilt: null,
	sources: {
		beam: "default" as const,
		color: "default" as const,
		dimmer: "default" as const,
		focus: "default" as const,
		position: "default" as const,
	},
	targetKind: "fixture" as const,
	tilt: 50,
	type: "Fixture",
};

const rows: FixtureSheetRow[] = [
	{
		...common,
		id: "101",
		fixtureId: "fixture-101",
		highlightBypassesGroupMaster: true,
		name: "Front Profile SL",
		fixtureType: "ETC · Source Four LED Series 3",
		icon: profileIcon,
		patch: "U1.1",
		dimmer: 72,
		color: "#f6c985",
		colorLabel: "Warm White",
		pan: 42,
		tilt: 38,
		positionLabel: "Lectern",
		beam: "Open",
		focus: "Sharp",
		sources: {
			...common.sources,
			dimmer: "programmer",
			color: "programmer",
			position: "programmer",
		},
	},
	{
		...common,
		id: "102.0",
		fixtureId: "fixture-102",
		limitingGroups: [storyFlashedGroup],
		name: "Stage Left Mover · Master",
		fixtureType: "Robe · Tetra2",
		icon: washIcon,
		patch: "U1.101",
		targetKind: "master",
		parentFixtureId: "fixture-102",
		childFixtureIds: ["fixture-102.1", "fixture-102.2"],
		dimmer: 48,
		color: "#1bd6ec",
		colorLabel: "Cyan",
		pan: 68,
		tilt: 27,
		positionLabel: "Downstage fan",
		beam: "Wash",
		focus: "Medium",
		sources: {
			...common.sources,
			dimmer: "default",
			color: "default",
			position: "default",
			beam: "playback",
			focus: "playback",
		},
	},
	{
		...common,
		id: "102.1",
		fixtureId: "fixture-102.1",
		name: "Stage Left Mover · Cell 1",
		fixtureType: "Robe · Tetra2 · Cell 1",
		icon: washIcon,
		patch: "U1.101",
		targetKind: "head",
		parentFixtureId: "fixture-102",
		indented: true,
		dimmer: 64,
		color: "#e24bdb",
		colorLabel: "Magenta",
		pan: 68,
		tilt: 27,
		positionLabel: "Downstage fan",
		beam: "Wash",
		focus: "Medium",
		preloadDimmer: 80,
		preloadColor: "#3f8cff",
		preloadPan: 72,
		preloadTilt: 30,
		sources: {
			...common.sources,
			dimmer: "programmer",
			color: "programmer",
			position: "default",
			beam: "playback",
			focus: "playback",
		},
	},
	{
		...common,
		id: "103",
		fixtureId: "fixture-103",
		name: "Back Wash",
		fixtureType: "Astera · AX9",
		icon: washIcon,
		patch: "U1.161",
		colorLabel: "White",
		positionLabel: "—",
		beam: "Wide",
		focus: "—",
		groupValues: {
			...storyGroupValues,
			focus: {
				id: "focus",
				members: [],
				available: false,
				source: "default",
				accessibleName: "Focus unavailable",
			},
		},
	},
];

const denseRows: FixtureSheetRow[] = Array.from({ length: 24 }, (_, index) => {
	if (index < rows.length) return rows[index];
	const fixtureNumber = 100 + index;
	const legacyProjection = index === 4;
	return {
		...rows[index % rows.length],
		id: String(fixtureNumber),
		fixtureId: `fixture-${fixtureNumber}`,
		name: `Fixture ${fixtureNumber}`,
		targetKind: "fixture" as const,
		parentFixtureId: "",
		childFixtureIds: [],
		indented: false,
		groupValues: legacyProjection
			? undefined
			: rows[index % rows.length]?.groupValues,
		beam: legacyProjection ? "Legacy Wide" : rows[index % rows.length]?.beam,
	} as FixtureSheetRow;
});

const presentStep: FixtureStepPresenter = (row) => ({
	base: row.fixtureId === "fixture-101",
	containedBase: false,
	current: row.fixtureId === "fixture-102.1",
	containedCurrent: row.fixtureId === "fixture-102",
});

function FixtureSheetStory({
	compact = false,
	compactMode = "off",
	small = false,
}: {
	compact?: boolean;
	compactMode?: FixtureSheetCompactMode;
	small?: boolean;
}) {
	const [activeRow, setActiveRow] = useState(1);
	const columns = useMemo(
		() =>
			fixtureSheetColumns(true, presentStep, compactMode).filter((column) =>
				(small
					? [
							"id",
							"name",
							"intensity",
							"color",
							"position",
							"beam",
							"shapers",
							"focus",
							"control",
							"media",
						]
					: DEFAULT_FIXTURE_SHEET_COLUMNS
				).includes(column.id as never),
			),
		[compactMode, small],
	);
	return (
		<div
			style={{
				height: small ? 360 : 680,
				width: small ? 430 : undefined,
				minWidth: small ? 430 : compact ? 420 : 760,
			}}
		>
			<FixtureSheetWindowView
				compact={compact}
				compactMode={compactMode}
				selectionCount={2}
				info={<SourceLegend />}
				table={
					<FixtureSheetTable
						activeRow={activeRow}
						columns={columns}
						onActivate={() => undefined}
						onActiveRowChange={setActiveRow}
						presentStep={presentStep}
						rows={small ? denseRows : rows}
						rowHeight={compactMode === "off" ? 43 : 32}
						selectedFixtureIds={new Set(["fixture-101", "fixture-102"])}
					/>
				}
			/>
		</div>
	);
}

const meta = {
	title: "ToskLight/Windows/Fixture Sheet",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const SelectedAndActiveSteps: Story = {
	render: () => <FixtureSheetStory />,
};

export const Compact: Story = {
	render: () => <FixtureSheetStory compact />,
};

export const IconOnly: Story = {
	render: () => <FixtureSheetStory compact compactMode="icon-only" small />,
};

export const TextOnly: Story = {
	render: () => <FixtureSheetStory compact compactMode="text-only" small />,
};

export const OffSmallScreen: Story = {
	render: () => <FixtureSheetStory compact compactMode="off" small />,
};
