import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	blankMode,
	blankChannel,
} from "@tosklight/patch/fixture-profile-model";
import type {
	NativeColorIdentity,
	InstalledColorCalibration,
} from "@tosklight/patch";
import { ColorCalibrationEditor } from "./ColorCalibration";
afterEach(cleanup);
function example() {
	const mode = blankMode();
	mode.heads[0].name = "Main";
	const channel = blankChannel(mode, 1);
	channel.fixture_attribute = "color.red";
	channel.attribute = "color.red";
	channel.functions = [
		{
			id: crypto.randomUUID(),
			name: "Red",
			attribute: "color.red",
			dmx_from: 0,
			dmx_to: 65535,
			priority: 0,
			behavior: {
				type: "continuous",
				physical_min: 0,
				physical_max: 1,
				unit: null,
			},
		},
	];
	channel.resolution = "u16";
	mode.channels = [channel];
	const path = {
		id: crypto.randomUUID(),
		head_id: mode.heads[0].id,
		controls: [channel.id],
		source: {
			type: "additive" as const,
			emitters: [
				{
					id: crypto.randomUUID(),
					name: "Red",
					binding: {
						channel_id: channel.id,
						function_id: channel.functions[0].id,
					},
					xyz: null,
					spectrum: [],
					band: "visible" as const,
					native_reversed: false,
					maximum_level: 1,
					response_exponent: 1,
					provenance: { quality: "unknown" as const, revision: 0 },
				},
			],
		},
		filters: [],
		measurements: [],
	};
	mode.color_physical = { version: 1, revision: 0, paths: [path] };
	const identity: NativeColorIdentity = {
		profile_id: crypto.randomUUID(),
		profile_revision: 2,
		profile_digest: "a".repeat(64),
		mode_id: mode.id,
		head_id: path.head_id,
		path_id: path.id,
		model_revision: 0,
		native_layout_signature: "b".repeat(64),
	};
	return { mode, path, identity };
}
describe("installed Color calibration editor", () => {
	it("authors a zero gain with exact server identity and saves only calibration", async () => {
		const { mode, identity } = example();
		const save = vi.fn().mockResolvedValue(true),
			close = vi.fn();
		render(
			<ColorCalibrationEditor
				identity="Fixture 1"
				initial={null}
				mode={mode}
				identities={[identity]}
				onSave={save}
				onClose={close}
			/>,
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Add Main emitter gain" }),
		);
		fireEvent.change(screen.getByLabelText("Main Red output gain"), {
			target: { value: "0" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
		expect(save.mock.calls[0][0].paths[0]).toMatchObject({
			source_identity: identity,
			emitters: [{ output_gain: 0, provenance: { quality: "unknown" } }],
		});
		await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
	});
	it("keeps stale measurements inactive until the operator clears them", async () => {
		const { mode, path, identity } = example();
		const save = vi.fn().mockResolvedValue(true);
		const initial: InstalledColorCalibration = {
			version: 1,
			revision: 4,
			paths: [
				{
					source_identity: { ...identity, profile_revision: 1 },
					emitters: [
						{
							emitter_id: path.source.emitters[0].id,
							output_gain: 0.8,
							provenance: {
								quality: "measured",
								source: "Old lamp",
								revision: 1,
							},
						},
					],
					measurements: [],
				},
			],
		};
		render(
			<ColorCalibrationEditor
				identity="Copy"
				initial={initial}
				mode={mode}
				identities={[identity]}
				onSave={save}
				onClose={() => {}}
			/>,
		);
		expect(screen.getByRole("status").textContent).toContain("inactive");
		expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
		fireEvent.click(
			screen.getByRole("button", { name: "Clear Color calibration" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() => expect(save).toHaveBeenCalledWith(null));
		expect(initial.paths[0].source_identity.profile_revision).toBe(1);
	});
	it("keeps native recipe precision and reports save failure without closing", async () => {
		const { mode, identity } = example();
		const save = vi.fn().mockResolvedValue(false),
			close = vi.fn();
		render(
			<ColorCalibrationEditor
				identity="Fixture 1"
				initial={null}
				mode={mode}
				identities={[identity]}
				onSave={save}
				onClose={close}
			/>,
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Add Main whole-path observation" }),
		);
		fireEvent.change(
			screen.getByLabelText("Main observation 1 color.red raw"),
			{ target: { value: "32769" } },
		);
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() =>
			expect(screen.getByRole("alert").textContent).toContain(
				"could not be saved",
			),
		);
		expect(save.mock.calls[0][0].paths[0].measurements[0].recipe[0].raw).toBe(
			32769,
		);
		expect(close).not.toHaveBeenCalled();
	});
});
