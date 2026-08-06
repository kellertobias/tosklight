import { fireEvent, render, screen } from "@testing-library/react";
import type {
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PreviewControls } from "./PreviewControls";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const PROFILE_ID = "profile-1";
const MODE_ID = "mode-1";
const HEAD_ID = "head-1";

/** A fixture patched at universe 1 address 1 from the profile revision below. */
function fixture(id: string, name: string): PatchFixtureProjection {
	return {
		fixtureRevision: 1,
		logicalHeads: [],
		fixtureId: id,
		fixtureNumber: 1,
		virtualFixtureNumber: null,
		name,
		profileId: PROFILE_ID,
		profileRevision: 1,
		modeId: MODE_ID,
		splitPatches: [{ split: 1, universe: 1, address: 1 }],
		layerId: "default",
		directControl: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		multipatch: [],
	} as unknown as PatchFixtureProjection;
}

/**
 * The revision the show embedded: a 16-bit intensity and a single-byte colour, so the slot list
 * has to show a coarse and a fine byte rather than one row per channel.
 */
const profileRevisions: PatchProfileRevision[] = [
	{
		profileId: PROFILE_ID,
		profileRevision: 1,
		contentDigest: "digest",
		manufacturer: "Acme",
		name: "Preview Wash",
		fixtureType: "wash",
		patchPolicy: "dmx",
		referencedModes: [
			{ modeId: MODE_ID, name: "Default", splits: [{ split: 1, footprint: 3 }] },
		],
		profileSnapshot: {
			schema_version: 2,
			id: PROFILE_ID,
			revision: 1,
			manufacturer: "Acme",
			name: "Preview Wash",
			short_name: "Preview",
			fixture_type: "wash",
			notes: "",
			photograph_asset: null,
			stage_icon_asset: null,
			model_asset: null,
			physical: {},
			modes: [
				{
					id: MODE_ID,
					name: "Default",
					notes: "",
					splits: [{ number: 1, footprint: 3 }],
					heads: [{ id: HEAD_ID, name: "Main", master_shared: true }],
					channels: [
						{
							id: "intensity",
							head_id: HEAD_ID,
							split: 1,
							attribute: "intensity",
							secondary_slots: [2],
						},
						{
							id: "red",
							head_id: HEAD_ID,
							split: 1,
							attribute: "color.red",
							secondary_slots: [],
						},
					],
					color_systems: [],
					control_actions: [],
				},
			],
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			reserved_source: null,
		},
	} as unknown as PatchProfileRevision,
];

const rig = [fixture("one", "Wash 1"), fixture("two", "Wash 2")];

beforeEach(() => {
	invoke.mockReset();
	invoke.mockResolvedValue(undefined);
});

describe("the preview controls", () => {
	it("says what to do when nothing is selected", () => {
		render(<PreviewControls
				fixtures={rig}
				profileRevisions={profileRevisions}
				selected={[]}
				onError={() => {}}
			/>);
		expect(
			screen.getByText(/Select fixtures in the patch sheet/i),
		).toBeInTheDocument();
	});

	it("sends a semantic value for every selected fixture", () => {
		render(
			<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one", "two"]} onError={() => {}} />,
		);
		fireEvent.change(screen.getByLabelText("Intensity"), { target: { value: "50" } });

		expect(invoke).toHaveBeenCalledWith("set_preview", {
			set: {
				kind: "semantic",
				fixture_id: "one",
				parameter: "intensity",
				value: 0.5,
				colour: [0, 0, 0],
			},
		});
		expect(invoke).toHaveBeenCalledWith("set_preview", {
			set: {
				kind: "semantic",
				fixture_id: "two",
				parameter: "intensity",
				value: 0.5,
				colour: [0, 0, 0],
			},
		});
	});

	it("offers Pan, Tilt, Intensity, Colour and Gobo, and nothing a desk would own", () => {
		render(<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one"]} onError={() => {}} />);
		for (const label of ["Intensity", "Pan", "Tilt", "Gobo", "Colour"]) {
			expect(screen.getByLabelText(label)).toBeInTheDocument();
		}
		// No programmer, no playbacks, no cue stack: this is not a second desk.
		expect(screen.queryByText(/Record|Cue|Playback/i)).not.toBeInTheDocument();
	});

	/// Full DMX is a testing tool for one fixture, so multi-selection has to disable it visibly.
	it("disables Full DMX unless exactly one fixture is selected", () => {
		const { rerender } = render(
			<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one", "two"]} onError={() => {}} />,
		);
		const full = screen.getByRole("tab", { name: "Full DMX" });
		expect(full).toBeDisabled();
		expect(full).toHaveAttribute(
			"title",
			"Full DMX needs exactly one fixture selected",
		);

		rerender(
			<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one"]} onError={() => {}} />,
		);
		expect(screen.getByRole("tab", { name: "Full DMX" })).toBeEnabled();
	});

	it("shows every slot of the mode, fine bytes included", () => {
		render(<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one"]} onError={() => {}} />);
		fireEvent.click(screen.getByRole("tab", { name: "Full DMX" }));

		expect(screen.getByLabelText("Slot 1 intensity (coarse)")).toBeInTheDocument();
		expect(screen.getByLabelText("Slot 2 intensity (fine)")).toBeInTheDocument();
		expect(screen.getByLabelText("Slot 3 color.red")).toBeInTheDocument();
	});

	it("sends a raw slot against the fixture's own footprint", () => {
		render(<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one"]} onError={() => {}} />);
		fireEvent.click(screen.getByRole("tab", { name: "Full DMX" }));
		fireEvent.change(screen.getByLabelText("Slot 2 intensity (fine)"), {
			target: { value: "200" },
		});

		expect(invoke).toHaveBeenCalledWith("set_preview", {
			set: { kind: "slot", fixture_id: "one", split: 1, offset: 2, value: 200 },
		});
	});

	/// Switching selection while in Full DMX must not leave a disabled mode showing.
	it("falls back to Simple when the selection grows", () => {
		const { rerender } = render(
			<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one"]} onError={() => {}} />,
		);
		fireEvent.click(screen.getByRole("tab", { name: "Full DMX" }));
		expect(screen.getByLabelText("Slot 1 intensity (coarse)")).toBeInTheDocument();

		rerender(
			<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["one", "two"]} onError={() => {}} />,
		);
		expect(screen.getByLabelText("Intensity")).toBeInTheDocument();
		expect(screen.queryByLabelText("Slot 1 intensity (coarse)")).not.toBeInTheDocument();
	});

	it("clears only the fixtures it is driving", () => {
		render(<PreviewControls fixtures={rig} profileRevisions={profileRevisions} selected={["two"]} onError={() => {}} />);
		fireEvent.click(screen.getByRole("button", { name: "Clear" }));
		expect(invoke).toHaveBeenCalledWith("clear_preview", { fixtures: ["two"] });
	});
});
