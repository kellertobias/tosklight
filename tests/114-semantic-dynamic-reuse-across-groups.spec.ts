// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { PresetFamily } from "./bench/groups-presets/presetScenario";
import { fixture } from "./bench/output/fixtureDmxContract";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

const MOVER = {
	manufacturer: "ROBE",
	profile: "Robin 300 LEDWash",
	mode: "Mode 3",
} as const;
const GROUP_ONE = [1, 2, 3, 4];
const GROUP_TWO = [5, 6, 7, 8];
/** Down and Up, as percentages of pan and tilt. */
const DOWN = { pan: 40, tilt: 20 };
const UP = { pan: 60, tilt: 80 };
const CYCLE_MILLIS = 4_000;

/** A byte range around a level, give or take a couple of DMX steps of rounding and interpolation. */
const near = (fraction: number) => ({
	between: [
		Math.max(0, Math.round(fraction * 255) - 3),
		Math.min(255, Math.round(fraction * 255) + 3),
	] as [number, number],
});

scenario(
	"BENCH-DYNAMIC-REUSE-001",
	"one targetless Dynamic applied to two mover Groups runs on each Group's own Virtual Playback and nowhere else",
	async (t) => {
		await t.show.use(Show.Empty);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		for (const number of [...GROUP_ONE, ...GROUP_TWO])
			await t.patch.via.api.add({
				number,
				name: `Mover ${number}`,
				address: `1.${1 + (number - 1) * 15}`,
				...MOVER,
			});
		await t.selection.fixtures.via.api.items(...GROUP_ONE);
		await t.group.via.api.store(1, { mode: StoreMode.Overwrite });
		await t.selection.fixtures.via.api.items(...GROUP_TWO);
		await t.group.via.api.store(2, { mode: StoreMode.Overwrite });
		await t.group.expect(1).fixtures(...GROUP_ONE);
		await t.group.expect(2).fixtures(...GROUP_TWO);

		// Down and Up hold the same positions for every mover in the rig.
		for (const [number, position] of [
			[1, DOWN],
			[2, UP],
		] as const) {
			await t.selection.fixtures.via.api.items(...GROUP_ONE, ...GROUP_TWO);
			await t.encoder.position.pan.via.api.set(position.pan);
			await t.encoder.position.tilt.via.api.set(position.tilt);
			await t.preset.via.api.store(PresetFamily.Position, number, {
				mode: "overwrite",
			});
			await t.encoder.clear();
		}
		await t.selection.clear();

		// Authored with nothing selected: the Dynamic names no Group and no fixtures.
		const dynamic = await t.dynamic.create({
			pool: 1,
			name: "Rise",
			cycleMillis: CYCLE_MILLIS,
			gridAngleDegrees: 0,
			lanes: [
				{ attribute: "intensity", keyframes: [[0, 0], [1, 1]] },
				...(["pan", "tilt"] as const).map((attribute) => ({
					attribute,
					keyframes: [
						[0, { preset: { family: PresetFamily.Position, number: 1 } }],
						[1, { preset: { family: PresetFamily.Position, number: 2 } }],
					] as Array<
						[number, { preset: { family: PresetFamily; number: number } }]
					>,
				})),
			],
		});
		await t.dynamic.expect(dynamic).targetless();

		// The same Dynamic, applied to each Group in turn and recorded onto its own Playback.
		for (const [index, group] of [1, 2].entries()) {
			await t.group.via.api.select(group);
			await t.dynamic.apply(dynamic);
			const playback = await t.record.playback(1 + index);
			await t.encoder.clear();
			await t.selection.clear();
			await t.playback.configure(playback, {
				name: `Rise ${group}`,
				buttonCount: 1,
				hasFader: false,
				buttons: [
					PlaybackButton.Toggle,
					PlaybackButton.Empty,
					PlaybackButton.Empty,
				],
			});
			await t.playback.via.api.off(playback);
		}
		// Recording it twice did not bind it to either Group.
		await t.dynamic.expect(dynamic).targetless();

		const desktop = t.desktop.configure("Dynamic reuse");
		const pane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{ slug: "dynamic-reuse", column: 1, row: 1, width: 12, height: 10 },
			{ rows: 1, columns: 2 },
		);
		await desktop.apply();
		const first = await t.virtualPlayback.assignSource(pane, "Rise 1", 1);
		const second = await t.virtualPlayback.assignSource(pane, "Rise 2", 2);

		/**
		 * Each member of a Group a quarter-cycle apart in selection order (no stage plan, so the grid
		 * spread falls back to it), `progress` of the way through its own cycle: the intensity lane
		 * rising from off to on and pan and tilt moving from Down to Up.
		 */
		const expectRising = async (members: number[], progress: number) => {
			for (const [index, number] of members.entries()) {
				const phase = (progress + index * 0.25) % 1;
				const between = (from: number, to: number) =>
					near((from + (to - from) * phase) / 100);
				await t.expectFixtureDMX(fixture(number), {
					Intensity: near(phase),
					"Tilt coarse": between(DOWN.tilt, UP.tilt),
					"Pan coarse": between(DOWN.pan, UP.pan),
				});
			}
		};
		const expectDark = async (members: number[]) => {
			for (const number of members)
				await t.expectFixtureDMX(fixture(number), { Intensity: 0 });
		};
		const quarter = `${CYCLE_MILLIS / 4}ms`;

		// Rise 1 runs the Dynamic on Group 1 only.
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.expect.runtime(first, { runtime: { enabled: true } });
		await t.virtualPlayback.expect.runtime(second, { runtime: { enabled: false } });
		await t.clock.advanceBy(quarter);
		await expectRising(GROUP_ONE, 0.25);
		await expectDark(GROUP_TWO);

		// Rise 2 starts the same Dynamic on Group 2, on its own clock, while Group 1 carries on.
		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.expect.runtime(first, { runtime: { enabled: true } });
		await t.virtualPlayback.expect.runtime(second, { runtime: { enabled: true } });
		await t.clock.advanceBy(quarter);
		await expectRising(GROUP_ONE, 0.5);
		await expectRising(GROUP_TWO, 0.25);

		// Each goes off on its own: Group 1 goes dark and Group 2 keeps rising.
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.expect.runtime(first, { runtime: { enabled: false } });
		await t.virtualPlayback.expect.runtime(second, { runtime: { enabled: true } });
		await t.clock.advanceBy(quarter);
		await expectDark(GROUP_ONE);
		await expectRising(GROUP_TWO, 0.5);

		await t.virtualPlayback.activate(pane, 2);
		await t.virtualPlayback.expect.runtime(second, { runtime: { enabled: false } });
		await t.clock.advanceStep();
		await expectDark([...GROUP_ONE, ...GROUP_TWO]);
		// Running it on both Groups still left the stored Dynamic bound to neither.
		await t.dynamic.expect(dynamic).targetless();
	},
);
