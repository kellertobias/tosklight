import { selectProgrammingGroup } from "./bench/command-selection/programmingSelection";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import {
	channels,
	colorReport,
	createDynamic,
	dynamicStatus,
	expectAngles,
	laneSources,
	outputDmx,
	playbackAction,
	poseAfter,
	readouts,
	seedDynamicPlayback,
	startDynamic,
	storedDynamic,
	toggleSpeedGroupPause,
} from "./bench/dynamics/intentFrameOutput";
import {
	activeShow,
	angles,
	arrangeRig,
	CMY,
	clearProgrammer,
	colorEdit,
	editPosition,
	type IntentRig,
	POINT,
	patchFixtures,
	pointTarget,
	presetBody,
	programColor,
	programmerRevision,
	programmerValues,
	RGB,
	RGBW,
	recall,
	recordPlayback,
	recordPreset,
	replaceFixture,
	requireSemanticFamilies,
	SPOT,
	select,
	setAngles,
	setEach,
	setIntensity,
	storedCues,
	undo,
	valuesAction,
	WASH,
} from "./bench/dynamics/intentFrameScenario";
import {
	clearPendingProgrammerPreload,
	enterProgrammerPreload,
	goProgrammerPreload,
} from "./bench/programmer/programmerPreloadLifecycle";

/**
 * docs/testing/32-intention-programming-frame-contract.md (TL-544, TL-548): the operator- and
 * API-observable parts of the intention programming frame contract, end to end through the real
 * server. Position is read in degrees from the typed readouts of the published Live (or Pending)
 * frame, Color from DMX and the accepted-frame Color report; the bench clock is manual, so every
 * frame is published by an explicit tick. Internal Rust seams the scenarios also name (registry
 * and journal cursors, transaction rollback, capacity) are covered by the Rust tests the
 * document's status section lists; PSN steps are covered by tests/123 (doc 19).
 */

type Keyframes = Array<[number, number]>;

/** A sweep of Pan only: its saved definition gains a Tilt partner that follows static Current. */
const PAN_SWEEP = {
	pool: 1,
	name: "Pan sweep",
	cycle: { speedGroupBeats: 8 },
	lanes: [{ component: "pan" as const, keyframes: [[0, -90], [0.5, 90], [0.999, -90]] as Keyframes }],
};
/** A complete pair, Tilt always 40° above Pan, so a mixed frame cannot hide. */
const CIRCLE = {
	pool: 2,
	name: "Circle",
	cycle: { millis: 2_000 },
	lanes: [
		{ component: "pan" as const, keyframes: [[0, 10], [0.5, 20], [0.999, 10]] as Keyframes },
		{ component: "tilt" as const, keyframes: [[0, 50], [0.5, 60], [0.999, 50]] as Keyframes },
	],
};
/** A constant complete pair at Pan 40° / Tilt 20°, bound to its own targets for a Playback. */
function holdAt(pool: number, mover: string) {
	return {
		pool,
		name: `Hold ${pool}`,
		cycle: { millis: 2_000 },
		targets: [mover],
		lanes: [
			{ component: "pan" as const, keyframes: [[0, 40], [0.999, 40]] as Keyframes },
			{ component: "tilt" as const, keyframes: [[0, 20], [0.999, 20]] as Keyframes },
		],
	};
}
/** The magenta the Color dialog writes for Hue 300° / Saturation 1, with UV at half. */
const MAGENTA_EDITS = [colorEdit("hue", 300), colorEdit("saturation", 1), colorEdit("uv", 0.5)];

async function moverRig(
	api: ApiDriver,
	label: string,
	count = 1,
): Promise<IntentRig & { movers: string[] }> {
	const rig = await arrangeRig(
		api,
		label,
		Array.from({ length: count }, (_, index) => ({
			number: index + 1,
			address: `1.${index * 20 + 1}`,
			...SPOT,
		})),
	);
	const movers = Array.from({ length: count }, (_, index) => rig.ids[index + 1]);
	await requireSemanticFamilies(api, movers);
	await select(api, rig, movers);
	return { ...rig, movers };
}

/** A mover aimed at a 3D Point that sits well away from it, so the Target solves to a real pose. */
async function aimedRig(api: ApiDriver, label: string) {
	const rig = await arrangeRig(api, label, [
		{ number: 1, address: "1.1", location: { x: 0, y: 6_000, z: 0 }, ...SPOT },
		{ number: 901, address: "2.1", location: { x: 3_000, y: 0, z: 2_000 }, ...POINT },
	]);
	const [mover, point] = [rig.ids[1], rig.ids[901]];
	await requireSemanticFamilies(api, [mover]);
	await select(api, rig, [mover]);
	await setEach(api, [mover], "position", pointTarget(point));
	return { rig, mover, point };
}

/** Frames of the Circle Dynamic: every Pan/Tilt pair keeps Tilt exactly 40° above Pan. */
async function expectCirclePairs(api: ApiDriver, bench: LightBench, mover: string, frames: number) {
	const pans = new Set<number>();
	for (let frame = 0; frame < frames; frame += 1) {
		const pose = await poseAfter(api, bench, mover, 125);
		expect(pose.pan, "Circle Pan").toBeGreaterThanOrEqual(10 - 0.01);
		expect(pose.pan, "Circle Pan").toBeLessThanOrEqual(20 + 0.01);
		expect(Math.abs(pose.tilt - pose.pan - 40), `frame ${JSON.stringify(pose)}`).toBeLessThan(0.02);
		pans.add(Math.round(pose.pan * 100));
	}
	if (frames > 1) expect(pans.size, "the Circle keeps moving").toBeGreaterThan(1);
}

function selectGroup(api: ApiDriver, rig: IntentRig, groupId: string) {
	return selectProgrammingGroup(api, {
		surface: "api",
		showId: rig.showId,
		groupId,
		frozen: false,
		rule: { type: "all" },
	});
}

test.describe("docs/testing/32-intention-programming-frame-contract.md", () => {
	test("INTENT-FRAME-001 @api › an Angle Dynamic owns the complete pair: Tilt follows static Current through an edit, a pause and save/reopen", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "001-current");
		const [mover] = rig.movers;
		await setAngles(api, [mover], 30, 15);
		const sweep = await createDynamic(api, rig, PAN_SWEEP);
		// Saving normalizes the pair: the untouched Tilt is a real partner lane sourced from Current.
		expect(laneSources(sweep, "tilt")?.length).toBeGreaterThan(0);
		expect(new Set(laneSources(sweep, "tilt"))).toEqual(new Set(["current"]));
		await startDynamic(api, rig, sweep.id);

		const start = await poseAfter(api, bench, mover, 0);
		const later = await poseAfter(api, bench, mover, 500);
		expect(Math.abs(later.pan - start.pan), "Pan sweeps").toBeGreaterThan(20);
		for (const pose of [start, later]) expect(Math.abs(pose.tilt - 15)).toBeLessThan(0.01);

		// An ordinary static Tilt edit: the Dynamic keeps its phase and Tilt follows −20°.
		expect((await editPosition(api, [mover], "tilt", -20)).status).toBe("changed");
		const edited = await poseAfter(api, bench, mover, 0);
		expectAngles(edited, later.pan, -20, "same phase after the edit");
		const moving = await poseAfter(api, bench, mover, 500);
		expect(Math.abs(moving.pan - edited.pan)).toBeGreaterThan(20);
		expect(Math.abs(moving.tilt + 20)).toBeLessThan(0.01);

		// Paused (Speed Group A drives the Dynamic): Pan holds while Tilt follows a new static 10°.
		expect((await toggleSpeedGroupPause(api)).snapshot.paused).toBe(true);
		const held = await poseAfter(api, bench, mover, 0);
		expect((await editPosition(api, [mover], "tilt", 10)).status).toBe("changed");
		expectAngles(await poseAfter(api, bench, mover, 500), held.pan, 10, "paused Pan, current Tilt");
		expect((await toggleSpeedGroupPause(api)).snapshot.paused).toBe(false);

		// Save/reopen: the partner is still Current, not a recorded copy of 15° or −20°.
		const other = await api.createShow<{ id: string }>({
			name: `INTENT-FRAME other ${crypto.randomUUID()}`,
		});
		await api.openShow(other.id, { transition: "hold_current" });
		await api.openShow(rig.showId, { transition: "hold_current" });
		await expect.poll(async () => (await activeShow(api))?.id).toBe(rig.showId);
		expect(new Set(laneSources(await storedDynamic(api, rig, sweep.id), "tilt"))).toEqual(
			new Set(["current"]),
		);
		await select(api, rig, [mover]);
		expect((await editPosition(api, [mover], "tilt", 5)).status).toBe("changed");
		const reopened = [await poseAfter(api, bench, mover, 0), await poseAfter(api, bench, mover, 500)];
		expect(Math.abs(reopened[1].pan - reopened[0].pan), "still sweeping after reopen").toBeGreaterThan(20);
		for (const pose of reopened) expect(Math.abs(pose.tilt - 5)).toBeLessThan(0.01);
	});

	test("INTENT-FRAME-001 @api › a later Angle Dynamic wins the whole pair; no frame joins one Dynamic's Pan to the other's Tilt", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "001-rank");
		const [mover] = rig.movers;
		await setAngles(api, [mover], 30, 15);
		const sweep = await createDynamic(api, rig, PAN_SWEEP);
		const circle = await createDynamic(api, rig, CIRCLE);
		await startDynamic(api, rig, sweep.id);
		await poseAfter(api, bench, mover, 250);
		// Started later at the same Programmer priority, the Circle ranks higher (LTP).
		await startDynamic(api, rig, circle.id);
		await expectCirclePairs(api, bench, mover, 8);
	});

	test("INTENT-FRAME-001 @api › a Pan FixAT masks only Pan while the Dynamic's Tilt runs on; removing it reveals the running phase", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "001-fixat");
		const [mover] = rig.movers;
		await setAngles(api, [mover], 5, 5);
		await recordPreset(api, rig, "Position", 1);
		await setAngles(api, [mover], 30, 15);
		const circle = await createDynamic(api, rig, CIRCLE);
		await startDynamic(api, rig, circle.id);
		await expectCirclePairs(api, bench, mover, 2);

		expect(
			await api.executeCommandLineRaw("FIXTURE 1 ATTRIBUTE PAN FIXAT POSITION PRESET 1"),
		).toMatchObject({ outcome: "accepted" });
		const tilts = new Set<number>();
		for (let frame = 0; frame < 4; frame += 1) {
			const pose = await poseAfter(api, bench, mover, 125);
			expect(Math.abs(pose.pan - 5), "Pan held by its explicit mask").toBeLessThan(0.01);
			expect(pose.tilt, "the unmasked Tilt stays the Circle's").toBeGreaterThanOrEqual(50 - 0.01);
			expect(pose.tilt).toBeLessThanOrEqual(60 + 0.01);
			tilts.add(Math.round(pose.tilt * 100));
		}
		expect(tilts.size, "Tilt keeps running under a Pan mask").toBeGreaterThan(1);

		// Removing the mask reveals the Dynamic at its current phase: a complete Circle pair again.
		expect((await undo(api, rig)).changed).toBe(true);
		await expectCirclePairs(api, bench, mover, 3);
	});

	test("INTENT-FRAME-001 @api › one representation owns Position: Target and Angles replace each other atomically", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "001-representation", 2);
		const origin = {
			kind: "position",
			value: {
				kind: "target",
				reference: { kind: "origin" },
				offset_metres: [1, 0, 2].map((value) => ({ kind: "value", value })),
			},
		};
		const kinds = async () =>
			(await programmerValues(api, "position")).map((entry) => entry.value.value.kind);
		await setEach(api, rig.movers, "position", origin);
		expect((await editPosition(api, rig.movers, "target_x", 1, "relative")).status).toBe("changed");
		expect(await kinds()).toEqual(["target", "target"]);
		await bench.tick(25);
		for (const pose of Object.values((await readouts(api, rig.movers)).poses))
			expect(pose.requested?.value.kind).toBe("target");

		// A Pan edit on a Target replaces it with complete Angles; no offset survives beside them.
		expect((await editPosition(api, rig.movers, "pan", 12)).status).toBe("changed");
		expect(await kinds()).toEqual(["angles", "angles"]);
		expect(JSON.stringify(await programmerValues(api, "position"))).not.toContain("offset_metres");
		await bench.tick(25);
		for (const pose of Object.values((await readouts(api, rig.movers)).poses)) {
			expect(pose.requested?.value.kind).toBe("angles");
			expect(Math.abs(pose.pan - 12)).toBeLessThan(0.01);
		}
	});

	test("INTENT-FRAME-001 @api › Angles stored in a preset, a Cue and a Group Cue recall in degrees on a replaced mover; Preload GO commits them", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "001-storage", 2);
		const [first, second] = rig.movers;
		await setAngles(api, rig.movers, 20, -30);
		await recordPreset(api, rig, "Position", 1);
		await recordPlayback(api, 1);
		expect(await api.executeCommandLineRaw("RECORD GROUP 10")).toMatchObject({ outcome: "accepted" });
		await clearProgrammer(api, rig);
		await selectGroup(api, rig, "10");
		const groupValue = await valuesAction(api, {
			type: "set_group",
			group_id: "10",
			attribute: "position",
			value: angles(-15, 25),
			timing: { fade: false },
		});
		expect(groupValue.status).toBe("changed");
		await recordPlayback(api, 2);
		await clearProgrammer(api, rig);
		await select(api, rig, []);

		// Every record holds degrees under the one Position owner, never normalized Pan/Tilt.
		const preset = await presetBody<{ values: Record<string, Record<string, unknown>> }>(
			api,
			rig,
			"Position",
			1,
		);
		for (const fixture of rig.movers) expect(preset.values[fixture]).toEqual({ position: angles(20, -30) });
		const cues = await storedCues(api, rig);
		const fixtureCue = cues.find((cue) => cue.changes.length > 0);
		const groupCue = cues.find((cue) => (cue.group_changes ?? []).length > 0);
		if (!fixtureCue || !groupCue) throw new Error(`both Cues are recorded: ${JSON.stringify(cues)}`);
		expect(fixtureCue.changes.map((change) => [change.attribute, change.value])).toEqual([
			["position", angles(20, -30)],
			["position", angles(20, -30)],
		]);
		expect(
			groupCue.group_changes?.map((change) => [change.group_id, change.attribute, change.value]),
		).toEqual([["10", "position", angles(-15, 25)]]);

		// Replace Mover 1 with a different mover: same fixture, other travel ranges.
		await replaceFixture(api, first, { number: 1, address: "1.1", ...WASH });
		const both = async (pan: number, tilt: number, label: string) => {
			await bench.tick(1_000);
			const poses = (await readouts(api, rig.movers)).poses;
			for (const fixture of rig.movers)
				expectAngles(poses[fixture], pan, tilt, `${label} ${fixture === first ? "replaced" : "original"}`);
		};
		await select(api, rig, rig.movers);
		await recall(api, rig, "Position", 1);
		await both(20, -30, "preset");
		await clearProgrammer(api, rig);
		await select(api, rig, []);
		await playbackAction(api, 1, "go");
		await both(20, -30, "Cue");
		await playbackAction(api, 1, "off");
		await playbackAction(api, 2, "go");
		await both(-15, 25, "Group Cue");
		await playbackAction(api, 2, "off");
		expect(await presetBody(api, rig, "Position", 1)).toEqual(preset);

		// Preload GO commits the prepared Angles to Live in one step.
		await enterProgrammerPreload(api, { surface: "api", showId: rig.showId });
		await select(api, rig, rig.movers);
		await setAngles(api, rig.movers, 40, 10, "preload");
		const live = await poseAfter(api, bench, second, 25);
		expect(Math.abs(live.pan - 40) > 0.5 || Math.abs(live.tilt - 10) > 0.5, "Live waits for GO").toBe(true);
		await goProgrammerPreload(api, { surface: "api", showId: rig.showId });
		await both(40, 10, "Preload GO");
	});

	test("INTENT-FRAME-002 @api › unavailable Target geometry withholds the Angle Dynamic as a complete pair while Intensity runs on, passively", async ({
		api,
		bench,
	}) => {
		const { rig, mover, point } = await aimedRig(api, "002-geometry");
		const aimed = await poseAfter(api, bench, mover, 25);
		expect(Math.abs(aimed.pan) + Math.abs(aimed.tilt), "the Point solves to a real aim").toBeGreaterThan(10);
		const sweep = await createDynamic(api, rig, {
			...PAN_SWEEP,
			cycle: { millis: 2_000 },
			scalarLanes: [{ attribute: "intensity", keyframes: [[0, 0], [0.5, 1], [0.999, 0]] }],
		});
		await startDynamic(api, rig, sweep.id);
		const running = [await poseAfter(api, bench, mover, 0), await poseAfter(api, bench, mover, 250)];
		expect(Math.abs(running[1].pan - running[0].pan), "Pan sweeps").toBeGreaterThan(20);
		for (const pose of running)
			expect(Math.abs(pose.tilt - aimed.tilt), "Tilt is the solved static Current").toBeLessThan(0.01);

		// The Point disappears: its Target can no longer be solved.
		await patchFixtures(api, [], [point]);
		const withheld = [];
		const levels = new Set<string>();
		for (let frame = 0; frame < 3; frame += 1) {
			withheld.push(await poseAfter(api, bench, mover, 250));
			levels.add((await channels(bench, 1, 20)).join(","));
		}
		for (const pose of withheld.slice(1)) {
			expect(pose.pan, "the Dynamic no longer drives Pan alone").toBeCloseTo(withheld[0].pan, 3);
			expect(pose.tilt, "nor a Tilt borrowed from elsewhere").toBeCloseTo(withheld[0].tilt, 3);
		}
		expect(levels.size, "Intensity keeps running").toBeGreaterThan(1);
		// The Programmer keeps the Target and its now-missing reference, unconverted.
		expect((await programmerValues(api, "position"))[0]?.value).toEqual(pointTarget(point));
	});

	test("INTENT-FRAME-002 @api › a Target whose Point is deleted holds the last valid aim instead of falling to zero", async ({
		api,
		bench,
	}) => {
		const { mover, point } = await aimedRig(api, "002-hold");
		const aimed = await poseAfter(api, bench, mover, 25);
		await patchFixtures(api, [], [point]);
		expectAngles(await poseAfter(api, bench, mover, 250), aimed.pan, aimed.tilt, "held aim");
	});

	test("INTENT-FRAME-002 @api › edits, recalls and FixAT with nothing selected are quiet no-ops that leave the Programmer unchanged", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "002-no-selection", 2);
		await setAngles(api, rig.movers, 10, 10);
		await recordPreset(api, rig, "Position", 1);
		await select(api, rig, []);
		const revision = await programmerRevision(api);
		const values = await programmerValues(api);

		expect((await editPosition(api, [], "tilt", -40)).status).toBe("no_change");
		const color = await valuesAction(api, {
			type: "apply_intent",
			fixture_ids: [],
			attribute: "color",
			operation: { type: "component_edits", edits: MAGENTA_EDITS },
			timing: { fade: false },
		});
		expect(color.status).toBe("no_change");
		expect(await api.executeCommandLineRaw("FIXAT POSITION PRESET 1")).toMatchObject({
			outcome: "accepted",
		});
		// With nothing selected, a preset recall is the documented full recall's first touch: it
		// selects the stored fixtures and applies nothing yet.
		expect(await recall(api, rig, "Position", 1)).toMatchObject({ appliedFixtures: 0 });
		await bench.tick(25);

		expect(await programmerRevision(api)).toBe(revision);
		expect(await programmerValues(api)).toEqual(values);
		const runtime = await api.request<{ instances: unknown[] }>(
			"GET",
			"/api/v2/dynamics/runtime",
			undefined,
			true,
			undefined,
			{ showId: rig.showId },
		);
		expect(runtime.instances, "no Dynamic was activated").toEqual([]);
	});

	test("INTENT-FRAME-002 @api › starting a Dynamic with nothing selected leaves the Programmer untouched", async ({
		api,
		bench,
	}) => {
		test.fail(true, "BUG: POST /api/v2/dynamics/{id}/start with an empty selection advances the Programmer values revision although nothing is activated");
		const rig = await moverRig(api, "002-no-selection-dynamic");
		const sweep = await createDynamic(api, rig, PAN_SWEEP);
		await select(api, rig, []);
		const revision = await programmerRevision(api);
		await api.request(
			"POST",
			`/api/v2/dynamics/${sweep.id}/start`,
			{
				request_id: crypto.randomUUID(),
				targets: [],
				overrides: { size: 1, speed_multiplier: { numerator: 1, denominator: 1 }, phase_offset_degrees: 0 },
				timing: {},
			},
			true,
			undefined,
			{ showId: rig.showId },
		);
		await bench.tick(25);
		expect(await programmerRevision(api)).toBe(revision);
	});

	test("INTENT-FRAME-002 @api › the Dynamic status does not report running semantic Angle lanes as skipped", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "002-status");
		const [mover] = rig.movers;
		await setAngles(api, [mover], 30, 15);
		const sweep = await createDynamic(api, rig, PAN_SWEEP);
		await startDynamic(api, rig, sweep.id);
		const [a, b] = [await poseAfter(api, bench, mover, 0), await poseAfter(api, bench, mover, 500)];
		expect(Math.abs(a.pan - b.pan), "the Dynamic demonstrably runs").toBeGreaterThan(20);
		const status = await dynamicStatus(api, rig, sweep.id);
		expect(status).toMatchObject({
			skipped_address_count: 0,
			supported_address_count: status?.lane_count,
		});
		expect(status?.warning ?? null).toBeNull();
	});

	test("INTENT-FRAME-003 @api › semantic Color, white target and UV survive RGB → RGBW → CMY replacement through presets and a Group Cue", async ({
		api,
		bench,
	}) => {
		const rig = await arrangeRig(api, "003-replacement", [
			{ number: 1, address: "1.1", ...RGB },
			{ number: 2, address: "1.21", ...SPOT },
		]);
		const [lamp, wheel] = [rig.ids[1], rig.ids[2]];
		const both = [lamp, wheel];
		await requireSemanticFamilies(api, both);
		await select(api, rig, both);
		await programColor(api, both, MAGENTA_EDITS);
		await setIntensity(api, both, 1);
		await recordPreset(api, rig, "Color", 1);
		const magenta = (await programmerValues(api, "color"))[0].value;
		expect(magenta).toMatchObject({
			value: { intent: { recipe: { rgb: [1, 0, 1] }, uv: { amount: 0.5 } } },
		});
		await programColor(api, both, [colorEdit("temperature", 3_200), colorEdit("white_blend", 1)]);
		await recordPreset(api, rig, "Color", 2);
		const warm = (await programmerValues(api, "color"))[0].value;
		expect(await api.executeCommandLineRaw("RECORD GROUP 10")).toMatchObject({ outcome: "accepted" });
		await clearProgrammer(api, rig);
		await selectGroup(api, rig, "10");
		for (const [attribute, value] of [
			["color", magenta],
			["intensity", { kind: "normalized", value: 1 }],
		] as const)
			await valuesAction(api, { type: "set_group", group_id: "10", attribute, value, timing: { fade: false } });
		await recordPlayback(api, 1);
		await clearProgrammer(api, rig);
		const records = async () => [
			await presetBody(api, rig, "Color", 1),
			await presetBody(api, rig, "Color", 2),
			await storedCues(api, rig),
		];
		const before = await records();

		// The wheel fixture reports its approximation and its unsupported UV passively.
		await select(api, rig, both);
		await recall(api, rig, "Color", 1);
		await setIntensity(api, both, 1);
		await bench.tick(25);
		const wheelHead = (await colorReport(api, rig, both)).heads.find((head) => head.fixture_id === wheel);
		expect(wheelHead?.has_target).toBe(true);
		expect(wheelHead?.quality).not.toBe("exact");
		expect(wheelHead?.uv?.status).toBe("unsupported");
		const wheelValue = (await programmerValues(api, "color")).find((value) => value.fixture_id === wheel);
		expect(wheelValue?.value, "UV stays stored").toEqual(magenta);

		// RGBW: the same magenta with White at zero; warm white drives White; magenta clears it again.
		await replaceFixture(api, lamp, { number: 1, address: "1.1", ...RGBW });
		await recall(api, rig, "Color", 1);
		await bench.tick(25);
		expect(await channels(bench, 1, 5)).toEqual([255, 255, 0, 255, 0]);
		await recall(api, rig, "Color", 2);
		await bench.tick(25);
		expect((await channels(bench, 1, 5))[4], "warm white uses the White emitter").toBeGreaterThan(0);
		const lampValue = (await programmerValues(api, "color")).find((value) => value.fixture_id === lamp);
		expect(lampValue?.value).toEqual(warm);
		await recall(api, rig, "Color", 1);
		await bench.tick(25);
		expect((await channels(bench, 1, 5))[4], "White cannot retain an unrelated value").toBe(0);

		// CMY: the Group Cue maps the same intent to subtractive filters.
		await replaceFixture(api, lamp, { number: 1, address: "1.1", ...CMY });
		await clearProgrammer(api, rig);
		await select(api, rig, []);
		await playbackAction(api, 1, "go");
		await bench.tick(1_000);
		expect(await channels(bench, 1, 4)).toEqual([255, 0, 255, 0]);
		await playbackAction(api, 1, "off");
		expect(await records(), "fitted output never replaces the semantic records").toEqual(before);
	});

	test("INTENT-FRAME-003 @api › a Direct recipe keeps its source identity and exact native values; replay on a different fixture is best effort", async ({
		api,
		bench,
	}) => {
		const PAR = { manufacturer: "Cameo", profile: "ROOT PAR 6", mode: "D7CH — Delay Off, virtual dimmer" } as const;
		const LUSTR = { manufacturer: "ETC", profile: "Source Four LED Series 2 Lustr", mode: "Direct" } as const;
		const rig = await arrangeRig(api, "003-direct", [{ number: 1, address: "1.1", ...PAR }]);
		const par = rig.ids[1];
		await requireSemanticFamilies(api, [par]);
		await select(api, rig, [par]);
		await programColor(api, [par], [colorEdit("hue", 0), colorEdit("saturation", 1)]);
		await bench.tick(25);
		const pages = await api.request<{
			reference?: { fixture_id: string; head_id: string } | null;
			pages: Array<{ controls: Array<{ channel_id: string; functions: Array<{ function_id: string }> }> }>;
		}>("GET", `/api/v2/programming/color/native-pages?fixture_ids=${par}`);
		const control = pages.pages[0]?.controls[0];
		expect(control && pages.reference, JSON.stringify(pages)).toBeTruthy();
		if (!control || !pages.reference) return;
		const native = await valuesAction(api, {
			type: "apply_intent",
			fixture_ids: [par],
			attribute: "color",
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "native",
						binding: { channel_id: control.channel_id, function_id: control.functions[0].function_id },
						operation: { kind: "relative", value: -7 },
					},
				],
			},
			timing: {},
			native_reference: { fixture_id: pages.reference.fixture_id, head_id: pages.reference.head_id },
		});
		expect(native.status, JSON.stringify(native)).toBe("changed");
		const direct = (await programmerValues(api, "color"))[0].value;
		expect(direct.value.kind).toBe("direct");
		await recordPreset(api, rig, "Color", 1);
		const stored = await presetBody(api, rig, "Color", 1);
		expect(stored).toMatchObject({
			universal_values: { color: { value: { kind: "direct", recipe: direct.value.recipe } } },
		});

		const replay = async () => {
			await clearProgrammer(api, rig);
			await select(api, rig, [par]);
			await recall(api, rig, "Color", 1);
			await setIntensity(api, [par], 1);
			await bench.tick(25);
			const head = (await colorReport(api, rig, [par])).heads.find((entry) => entry.fixture_id === par);
			return { value: (await programmerValues(api, "color"))[0]?.value, replay: head?.direct?.replay };
		};
		expect(await replay()).toEqual({ value: direct, replay: "exact" });
		await replaceFixture(api, par, { number: 1, address: "1.1", ...LUSTR });
		const elsewhere = await replay();
		expect(elsewhere.value, "the original source identity and raw values stay stored").toEqual(direct);
		expect(["fallback", "native_only"]).toContain(elsewhere.replay);
		expect(await presetBody(api, rig, "Color", 1)).toEqual(stored);
	});

	test("INTENT-FRAME-004 @api › DMX, Position readouts and the Color report publish one frame identity; reading is inert", async ({
		api,
		bench,
	}) => {
		const rig = await arrangeRig(api, "004-frame", [
			{ number: 1, address: "1.1", ...SPOT },
			{ number: 2, address: "1.21", ...RGB },
		]);
		const ids = [rig.ids[1], rig.ids[2]];
		await requireSemanticFamilies(api, ids);
		await select(api, rig, ids);
		await setAngles(api, [rig.ids[1]], -25, 35);
		await programColor(api, ids, MAGENTA_EDITS);
		await setIntensity(api, ids, 1);
		const revision = (await activeShow(api))?.revision;
		await bench.tick(25);

		const read = async () => ({
			dmx: await outputDmx(api),
			position: await readouts(api, ids),
			color: await colorReport(api, rig, ids),
		});
		const first = await read();
		expect(first.dmx.frame).not.toBeNull();
		expect(first.position.frame).toEqual(first.dmx.frame);
		expect(first.color.accepted_frame).toMatchObject({ state: "accepted", frame: first.dmx.frame });
		expectAngles(first.position.poses[rig.ids[1]], -25, 35);

		// Repeated and duplicate reads neither publish nor queue frames, and never write the show.
		for (const reading of await Promise.all([read(), read(), read()])) {
			expect(reading.dmx.frame).toEqual(first.dmx.frame);
			expect(reading.dmx.universes).toEqual(first.dmx.universes);
		}
		await bench.tick(25);
		const next = await read();
		expect(next.dmx.frame?.sequence).toBe((first.dmx.frame?.sequence ?? 0) + 1);
		expect(next.position.frame).toEqual(next.dmx.frame);
		expect((await activeShow(api))?.revision).toBe(revision);
	});

	test("INTENT-FRAME-004 @api › a scalar Dynamic on a Point axis moves the Point once and the aimed mover follows in the same frame without writing the show", async ({
		api,
		bench,
	}) => {
		const { rig, mover, point } = await aimedRig(api, "004-point");
		const resting = await poseAfter(api, bench, mover, 25);
		await select(api, rig, [point]);
		const slide = await createDynamic(api, rig, {
			pool: 3,
			name: "Truss slide",
			cycle: { millis: 2_000 },
			lanes: [],
			scalarLanes: [{ attribute: "point.position.x", keyframes: [[0, 0.3], [0.5, 0.7], [0.999, 0.3]] }],
		});
		await startDynamic(api, rig, slide.id);
		const revision = (await activeShow(api))?.revision;

		// 0.3 → 0.4 → 0.5 of the axis: −40 m, −20 m, 0 m. Applied once, never doubled.
		const aims = [];
		for (const [step, offset] of [[0, -40], [250, -20], [250, 0]] as const) {
			await bench.tick(step);
			const [dmx, position] = [await outputDmx(api), await readouts(api, [mover])];
			expect(position.frame, "one frame for DMX, Point poses and readouts").toEqual(dmx.frame);
			const pose = dmx.points.find((candidate) => candidate.fixture_id === point);
			expect(pose?.offset_metres[0]).toBeCloseTo(offset, 2);
			aims.push(position.poses[mover]);
		}
		expect(Math.abs(aims[0].pan - aims[1].pan), "the aim follows the moving Point").toBeGreaterThan(1);
		expectAngles(aims[2], resting.pan, resting.tilt, "back at rest");
		expect((await activeShow(api))?.revision, "motion never writes the show").toBe(revision);
	});

	test("INTENT-FRAME-005 @api › Preload has its own frame; Live is untouched until Preload GO, and Shift Preload clears", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "005-preload");
		const [mover] = rig.movers;
		await setAngles(api, [mover], 0, 15);
		const sweep = await createDynamic(api, rig, PAN_SWEEP);
		await startDynamic(api, rig, sweep.id);
		await poseAfter(api, bench, mover, 0);
		const prepare = async () => {
			await enterProgrammerPreload(api, { surface: "api", showId: rig.showId });
			await select(api, rig, [mover]);
			await setAngles(api, [mover], 40, -30, "preload");
		};

		await prepare();
		await expect
			.poll(
				async () => {
					await bench.tick(25);
					return (await readouts(api, [mover], "preload")).frame !== null;
				},
				{ timeout: 5_000 },
			)
			.toBe(true);
		const preview = await readouts(api, [mover], "preload");
		const live = await readouts(api, [mover]);
		expect(preview.frame, "Preload stamps its own evaluation").not.toEqual(live.frame);
		expect(Math.abs(preview.poses[mover].tilt + 30), "pending static Tilt").toBeLessThan(0.01);
		const liveFrames = [await poseAfter(api, bench, mover, 250), await poseAfter(api, bench, mover, 250)];
		for (const pose of liveFrames)
			expect(Math.abs(pose.tilt - 15), "Live keeps its own static Tilt").toBeLessThan(0.01);
		expect(Math.abs(liveFrames[1].pan - liveFrames[0].pan), "the Live effect runs on").toBeGreaterThan(20);

		// Reading either lane dispatches nothing.
		const revisions = async () => [await programmerRevision(api), (await outputDmx(api, true)).revision];
		const quiet = await revisions();
		await Promise.all([readouts(api, [mover], "preload"), readouts(api, [mover]), outputDmx(api, true)]);
		expect(await revisions()).toEqual(quiet);

		// Shift Preload: the pending episode is discarded and Live is unchanged.
		await clearPendingProgrammerPreload(api, { surface: "api", showId: rig.showId });
		expect(Math.abs((await poseAfter(api, bench, mover, 250)).tilt - 15)).toBeLessThan(0.01);

		// Preload GO: the pending static Tilt becomes Live's static Current under the running effect.
		await prepare();
		await goProgrammerPreload(api, { surface: "api", showId: rig.showId });
		await bench.tick(25);
		const committed = [await poseAfter(api, bench, mover, 250), await poseAfter(api, bench, mover, 250)];
		for (const pose of committed) expect(Math.abs(pose.tilt + 30), "committed static Tilt").toBeLessThan(0.01);
		expect(Math.abs(committed[1].pan - committed[0].pan), "the effect keeps its Pan").toBeGreaterThan(20);
	});

	test("INTENT-FRAME-006 @api › a Dynamic Playback master crossfades from static Current only with crossfade enabled", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "006-master");
		const [mover] = rig.movers;
		await setAngles(api, [mover], -40, -20);
		await recordPlayback(api, 1);
		await clearProgrammer(api, rig);
		await select(api, rig, []);
		await playbackAction(api, 1, "go");
		expectAngles(await poseAfter(api, bench, mover, 1_000), -40, -20, "static Current");
		await seedDynamicPlayback(api, rig, 3, { ...(await createDynamic(api, rig, holdAt(3, mover))), pool_number: 3 }, true);
		await seedDynamicPlayback(api, rig, 4, { ...(await createDynamic(api, rig, holdAt(4, mover))), pool_number: 4 }, false);
		// The virtual fader of each standalone Dynamic Playback (see the physical-fader bug below).
		const master = async (playback: number, value: number) => {
			await playbackAction(api, playback, "master", { value, surface: "virtual" });
			return poseAfter(api, bench, mover, 1_000);
		};
		// Crossfade: the endpoint blends from static Current; zero is a Current-valued vote.
		expectAngles(await master(3, 1), 40, 20, "crossfade full");
		expectAngles(await master(3, 0.5), 0, 0, "crossfade half");
		expectAngles(await master(3, 0.25), -20, -10, "crossfade quarter");
		expectAngles(await master(3, 0), -40, -20, "crossfade zero");
		await playbackAction(api, 3, "off");
		// No crossfade: a positive master leaves the endpoint intact; zero casts no vote.
		expectAngles(await master(4, 1), 40, 20, "no crossfade full");
		expectAngles(await master(4, 0.25), 40, 20, "no crossfade quarter");
		expectAngles(await master(4, 0), -40, -20, "no crossfade zero");
		await playbackAction(api, 4, "off");
	});

	test("INTENT-FRAME-006 @api › the physical fader of a Dynamic Playback moves its master", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "006-physical");
		const [mover] = rig.movers;
		await select(api, rig, []);
		const dynamic = await createDynamic(api, rig, holdAt(3, mover));
		await seedDynamicPlayback(api, rig, 3, { ...dynamic, pool_number: 3 }, true);
		await playbackAction(api, 3, "master", { value: 1 });
		// The runtime instance starts on the next output frame (see the burst bug below).
		await bench.tick(25);
		await playbackAction(api, 3, "master", { value: 0.5 });
		expectAngles(await poseAfter(api, bench, mover, 1_000), 20, 10, "half master from home");
	});

	test("INTENT-FRAME-006 @api › a fader burst before the next frame still starts the Dynamic Playback", async ({
		api,
		bench,
	}) => {
		test.fail(
			true,
			"BUG: a second master move before the next output frame leaves the Dynamic runtime instance unstarted (state failed) for good",
		);
		const rig = await moverRig(api, "006-burst");
		const [mover] = rig.movers;
		await select(api, rig, []);
		const dynamic = await createDynamic(api, rig, holdAt(3, mover));
		await seedDynamicPlayback(api, rig, 3, { ...dynamic, pool_number: 3 }, true);
		await playbackAction(api, 3, "master", { value: 1 });
		await playbackAction(api, 3, "master", { value: 0.5 });
		expectAngles(await poseAfter(api, bench, mover, 1_000), 20, 10, "half master from home");
	});

	test("INTENT-FRAME-006 @api › Cue brightness comes through Intensity once; stored Color is never faded toward black", async ({
		api,
		bench,
	}) => {
		const rig = await arrangeRig(api, "006-color", [{ number: 1, address: "1.1", ...RGB }]);
		const lamp = rig.ids[1];
		await requireSemanticFamilies(api, [lamp]);
		await select(api, rig, [lamp]);
		await programColor(api, [lamp], [colorEdit("hue", 300), colorEdit("saturation", 1)]);
		await setIntensity(api, [lamp], 1);
		await recordPlayback(api, 1);
		await clearProgrammer(api, rig);
		await select(api, rig, []);
		await playbackAction(api, 1, "go");
		await bench.tick(1_000);
		expect(await channels(bench, 1, 4)).toEqual([255, 255, 0, 255]);
		// A fresh fader picks the master up at Full before it moves it.
		await playbackAction(api, 1, "master", { value: 1 });
		for (const [level, dimmer] of [
			[0.5, 128],
			[0, 0],
		] as const) {
			await playbackAction(api, 1, "master", { value: level });
			await bench.tick(25);
			expect(await channels(bench, 1, 4), `master ${level}`).toEqual([dimmer, 255, 0, 255]);
		}
	});

	test("INTENT-FRAME-006 @api › a Pan FixAT keeps its own value while the Dynamic Playback master crossfades the unmasked Tilt", async ({
		api,
		bench,
	}) => {
		const rig = await moverRig(api, "006-fixat");
		const [mover] = rig.movers;
		await setAngles(api, [mover], 5, 5);
		await recordPreset(api, rig, "Position", 1);
		await setAngles(api, [mover], -40, -20);
		await recordPlayback(api, 1);
		await clearProgrammer(api, rig);
		await select(api, rig, []);
		await playbackAction(api, 1, "go");
		const dynamic = await createDynamic(api, rig, holdAt(3, mover));
		await seedDynamicPlayback(api, rig, 3, { ...dynamic, pool_number: 3 }, true);
		await playbackAction(api, 3, "master", { value: 1, surface: "virtual" });
		await playbackAction(api, 3, "master", { value: 0.5, surface: "virtual" });
		expectAngles(await poseAfter(api, bench, mover, 1_000), 0, 0, "half master");
		expect(
			await api.executeCommandLineRaw("FIXTURE 1 ATTRIBUTE PAN FIXAT POSITION PRESET 1"),
		).toMatchObject({ outcome: "accepted" });
		expectAngles(await poseAfter(api, bench, mover, 1_000), 5, 0, "masked Pan, mastered Tilt");
		await playbackAction(api, 3, "master", { value: 0.25, surface: "virtual" });
		expectAngles(await poseAfter(api, bench, mover, 1_000), 5, -10, "the mask ignores the master");
	});
});
