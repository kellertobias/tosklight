// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { fixture } from "./bench/command-selection/selectionContract";
import { PresetFamily } from "./bench/groups-presets/presetScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"PERF-PROGRAMMER-001",
	"programmer actions acknowledge and reach their first output frame within the output-tick budget",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		const timing = t.programmerActionTiming;

		await timing.expectAction(
			{
				source: "http",
				route: "http",
				action: "command_line_edit",
				requiresOutputFrame: false,
			},
			() => t.command.via.api.type("FIXTURE 1"),
		);
		await timing.expectAction(
			{
				source: "http",
				route: "http",
				action: "selection",
				requiresOutputFrame: false,
			},
			() => t.selection.fixtures.via.api.item(1),
		);
		await timing.expectAction(
			{
				source: "http",
				route: "http",
				action: "values",
				requiresOutputFrame: true,
			},
			() => t.encoder.intensity.dimmer.via.api.set(20),
		);

		await timing.expectLiveAction("command_line_edit", false, {
			type: "command_line_set",
			request: { value: "FIXTURE 1 AT 30" },
		});
		await timing.expectHttpCommandKey();
		await timing.expectLiveAction("command_execute", true, {
			type: "command_line_execute",
			request: { value: "FIXTURE 1 AT 30" },
		});
		await timing.expectKeyboardCommand("FIXTURE 1 AT 35");

		await timing.expectAction(
			{
				source: "websocket",
				route: "software",
				action: "preload_lifecycle",
				requiresOutputFrame: true,
			},
			() => t.keypad.press(["PRE"]),
		);
		await t.keypad.press(["ESC"]);

		await timing.expectLiveAction("undo", true, {
			type: "programmer_undo",
		});

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(45);
		await t.preset.via.api.store(PresetFamily.Intensity, 99, {
			mode: "overwrite",
		});
		await t.encoder.clear();
		await timing.expectAction(
			{
				source: "http",
				route: "http",
				action: "preset_recall",
				requiresOutputFrame: true,
			},
			() => t.preset.via.api.recall(PresetFamily.Intensity, 99),
		);

		await timing.expectDynamicStart(t.show.contractIdentity().workingId);
		await timing.expectDirectOscCommandEdit();

		await t.hardware.connect();
		try {
			await timing.expectOscProgrammerKey(t.hardware, "digit-8", false);
		} finally {
			await t.hardware.disconnect();
		}

		await timing.setOutputFrameRate(120);
		await timing.expectLiveAction(
			"command_execute",
			true,
			{
				type: "command_line_execute",
				request: { value: "FIXTURE 1 AT 55" },
			},
			"websocket",
		);
		await t.expectFixtureDMX(fixture(1), { Intensity: 140 });

		timing.expectCoverage({
			routes: [
				"software",
				"keyboard",
				"http",
				"websocket",
				"osc",
				"attached-hardware",
			],
			actions: [
				"command_line_edit",
				"selection",
				"values",
				"command_execute",
				"command_key",
				"undo",
				"preset_recall",
				"dynamic",
				"programmer_key",
			],
			frameRateBands: ["at-or-below-60", "above-60"],
		});
	},
	{ tags: ["@performance", "@programmer"] },
);
