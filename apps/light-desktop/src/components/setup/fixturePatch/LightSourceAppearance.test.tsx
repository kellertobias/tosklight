import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	blankChannel,
	blankMode,
} from "@tosklight/patch/fixture-profile-model";
import type { MultiPatchInstance, PatchedFixture } from "../../../api/types";
import type { PatchController } from "./controller";
import { ColorCalibrationDialog } from "./ColorCalibration";
import { LightSourceCell } from "./LightSourceAppearance";

let controller: PatchController;
vi.mock("./controller", () => ({ usePatchController: () => controller }));
afterEach(cleanup);

function setup(copy: boolean, armed = true, color = true) {
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
			dmx_to: 255,
			priority: 0,
			behavior: {
				type: "continuous",
				physical_min: 0,
				physical_max: 1,
				unit: null,
			},
		},
	];
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
	mode.color_physical = color
		? { version: 1, revision: 0, paths: [path] }
		: null;
	const identity = {
		profile_id: crypto.randomUUID(),
		profile_revision: 1,
		profile_digest: "a".repeat(64),
		mode_id: mode.id,
		head_id: path.head_id,
		path_id: path.id,
		model_revision: 0,
		native_layout_signature: "b".repeat(64),
	};
	const instance = {
		id: "physical-copy",
		name: "Opposite hang",
		color_calibration: null,
	} as MultiPatchInstance;
	const fixture = {
		fixture_id: "root",
		fixture_number: 3,
		color_calibration: null,
		multipatch: [instance],
		definition: {
			mode_id: mode.id,
			profile_snapshot: { modes: [mode] },
			color_calibration_context: { mode, identities: color ? [identity] : [] },
		},
	} as unknown as PatchedFixture;
	const save = vi.fn().mockResolvedValue(true);
	const selectPatchInstance = vi.fn(),
		setAppearanceEdit = vi.fn(),
		setSelectedFixture = vi.fn();
	function Harness() {
		const [target, setTarget] = useState<{
			fixtureId: string;
			multipatchInstanceId: string | null;
		} | null>(null);
		controller = {
			data: { all: [fixture] },
			appState: { patchSetArmed: armed },
			ui: {
				colorCalibrationEdit: target,
				setColorCalibrationEdit: setTarget,
				setSelectedFixture,
				setAppearanceEdit,
			},
			patch: { selectPatchInstance, updateFixtureIntent: save },
		} as unknown as PatchController;
		return (
			<>
				<table>
					<tbody>
						<tr>
							<LightSourceCell
								fixture={fixture}
								instance={copy ? instance : undefined}
							/>
						</tr>
					</tbody>
				</table>
				<ColorCalibrationDialog />
			</>
		);
	}
	render(<Harness />);
	return { fixture, instance, save };
}

describe("Color calibration without Stage emitters", () => {
	it.each([
		false,
		true,
	])("opens and saves only the exact physical target (copy=%s)", async (copy) => {
		const { fixture, instance, save } = setup(copy);
		expect(screen.getByText("No emitter")).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", {
				name: `Color calibration ${copy ? "Opposite hang" : "3"}`,
			}),
		);
		expect(
			screen.getByRole("dialog", {
				name: `Color calibration ${copy ? "Opposite hang" : "3"}`,
			}),
		).toBeInTheDocument();
		expect(controller.ui.setAppearanceEdit).not.toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Add Main whole-path observation" }),
		);
		fireEvent.change(screen.getByLabelText("Main observation 1 Y"), {
			target: { value: "0.42" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() =>
			expect(save).toHaveBeenCalledWith("root", copy ? "physical-copy" : null, {
				type: "set_color_calibration",
				calibration: expect.objectContaining({
					paths: [
						expect.objectContaining({
							measurements: [
								expect.objectContaining({ xyz: { x: 0, y: 0.42, z: 0 } }),
							],
						}),
					],
				}),
			}),
		);
		expect(fixture.color_calibration).toBeNull();
		expect(instance.color_calibration).toBeNull();
	});
	it.each([
		false,
		true,
	])("keeps SET arming and explicit context-menu access (copy=%s)", (copy) => {
		setup(copy, false);
		const button = screen.getByRole("button", {
			name: `Color calibration ${copy ? "Opposite hang" : "3"}`,
		});
		fireEvent.click(button);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		fireEvent.contextMenu(button);
		expect(screen.getByRole("dialog")).toBeInTheDocument();
		expect(controller.patch.selectPatchInstance).toHaveBeenCalledWith({
			fixtureId: "root",
			multipatchInstanceId: copy ? "physical-copy" : null,
		});
	});
	it.each([
		false,
		true,
	])("keeps fixtures without Color paths honestly unavailable (copy=%s)", (copy) => {
		setup(copy, true, false);
		expect(screen.getByText("No emitter")).toBeInTheDocument();
		expect(screen.queryByRole("button")).not.toBeInTheDocument();
	});
});
