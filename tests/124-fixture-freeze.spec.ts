import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { loadCanonicalCopy } from "./support/catalog";
import { simulatedHardware } from "./bench/hardware/hardwareScenario";
import { poolAction } from "./bench/playbacks/playback-configuration/api";
import { BrowserPatch } from "./bench/show-setup/patchScenario";
import {
	arrangeFreezeRig,
	changeEverySource,
	commandLineText,
	cueListBody,
	DIMMER_SLOTS,
	FixtureSheetFreeze,
	FREEZE_GROUP,
	FreezeKeypad,
	FreezeOscKeys,
	type FreezeRig,
	freezeTargets,
	outputFrame,
	PAR_SLOTS,
	patchFixtures,
	patchRevision,
	programAngles,
	programColor,
	programmerIntensities,
	setZoom,
	setGlobalMaster,
	setGroupMaster,
	setIntensity,
	slots,
	visualizationValues,
	WASH_HEAD_COLOR_SLOTS,
	WASH_MASTER_SLOTS,
} from "./bench/specific-features/fixtureFreezeScenario";

/**
 * docs/testing/17-fixture-freeze.md: Freeze retains resolved fixture output without rewriting the
 * Programmer, Cue, Dynamic or master state, across the keypad, OSC and attached-hardware grammar.
 *
 * The rig (`arrangeFreezeRig`) places three intensity fixtures (Programmer, Cue and Dynamic driven),
 * a multi-head ROBE Robin 600X LEDWash and a single-head Cameo ROOT PAR 6 in Group 10, under a
 * Group Master at 80 %, the Grand Master at 90 % and Blackout off. Physical output is read from the
 * bench Art-Net receiver, checked against the logical frame of the same manual-clock step.
 */

const GROUP_KEYS = ["GRP", "1", "0"];

async function fullFreezeGroup(api: ApiDriver) {
	await api.executeCommandLine(`FREEZE GROUP ${FREEZE_GROUP}`);
}

async function frameAfterSourcesChange(
	api: ApiDriver,
	bench: LightBench,
	rig: FreezeRig,
) {
	await changeEverySource(api, bench, rig);
	return outputFrame(bench);
}

/** An 8-bit DMX level within one step of an exact product of levels (rounding differs by path). */
function expectDmxNear(actual: number, exact: number) {
	expect(Math.abs(actual - exact), `DMX ${actual} vs ${exact.toFixed(2)}`).toBeLessThanOrEqual(1);
}

/** Changes the Intensity, Color, Position and Beam sources, leaving every master unchanged. */
async function changeSourcesOnly(
	api: ApiDriver,
	bench: LightBench,
	rig: FreezeRig,
) {
	await setIntensity(api, rig.showId, rig.dimmers[0], 0.1);
	await setIntensity(api, rig.showId, rig.wash, 0.3);
	await setIntensity(api, rig.showId, rig.par, 0.3);
	await programColor(api, [...rig.washHeads, rig.par], 240);
	await programAngles(api, rig.showId, rig.wash, -60, -20);
	await setZoom(api, rig.showId, rig.wash, 40);
	await poolAction(api, rig.cuePlayback, "go");
	await bench.tick(1_300);
}

/** The server-owned live action of the family Freeze keys (Toggle). */
async function toggleFamilies(api: ApiDriver, families: string[]) {
	await api.request("POST", "/api/v2/fixture-freeze/actions", { families });
}

test.describe("docs/testing/17-fixture-freeze.md", () => {
	test("FREEZE-FULL-001 @ui › keypad Freeze holds intensity fixtures through every source, master and Blackout; Unfreeze reveals the untouched underlying state", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		test.setTimeout(90_000);
		const rig = await arrangeFreezeRig({ api, bench, desk, page, show });
		const captured = await outputFrame(bench, 0);
		const capturedDimmers = slots(captured, DIMMER_SLOTS);
		// Every source contributes and every master is below Full: 60 % x 80 % x 90 %, the Cue's 50 %
		// and the Dynamic, all visibly non-zero.
		expectDmxNear(capturedDimmers[0], 255 * 0.6 * 0.8 * 0.9);
		expectDmxNear(capturedDimmers[1], 255 * 0.5 * 0.8 * 0.9);
		expect(capturedDimmers[2]).toBeGreaterThan(0);
		const cueList = await cueListBody(api, rig.showId, rig.cueListId);

		await desk.open(bench.baseUrl);
		const sheet = new FixtureSheetFreeze(page);
		await sheet.open();
		const keypad = new FreezeKeypad(page, desk);
		await keypad.enter("FREEZE", GROUP_KEYS, [], "FREEZE G10");

		await expect
			.poll(async () => Object.keys(await freezeTargets(api)).length)
			// Three dimmers, the wash root (its Master channels) and its three heads, the PAR.
			.toBe(3 + 1 + 3 + 1);
		const targets = await freezeTargets(api);
		for (const fixtureId of [...rig.dimmers, rig.wash, ...rig.washHeads, rig.par])
			expect(targets[fixtureId], fixtureId).toEqual({
				full: true,
				families: [],
			});
		for (const name of [
			"Dimmer 1",
			"Dimmer 2",
			"Dimmer 3",
			"Wash 101 · zone 1",
			"Wash 101 · zone 2",
			"Wash 101 · zone 3",
			"Par 201",
		])
			await expect(sheet.status(name), name).toHaveText("❄ FREEZE");
		// The wash's own Master channels (Pan, Tilt, Shutter, Intensity) are frozen with its heads.
		await expect(sheet.status("Wash 101 · Master")).toHaveText("❄ FREEZE");
		await expect(sheet.status("Dimmer 4")).toHaveCount(0);
		// The Programmer and Cue dimmers keep the exact pre-Freeze frame. The Dynamic dimmer's capture
		// is compared from the first frozen frame: with the desk UI attached it was seen to sample a
		// different Dynamic phase than the bench's last manual-clock frame (1 in 3 runs).
		const frozenDimmers = slots(await outputFrame(bench, 0), DIMMER_SLOTS);
		expect(frozenDimmers.slice(0, 2)).toEqual(capturedDimmers.slice(0, 2));
		const frozenVisualization = await visualizationValues(api, rig.dimmers, [
			"intensity",
		]);

		const changed = await frameAfterSourcesChange(api, bench, rig);
		expect(
			slots(changed, DIMMER_SLOTS),
			"frozen physical output stays at the captured frame",
		).toEqual(frozenDimmers);
		expect(
			await visualizationValues(api, rig.dimmers, ["intensity"]),
		).toEqual(frozenVisualization);
		// Unfrozen Dimmer 4 proves Blackout really is on.
		expect(slots(changed, [4])).toEqual([0]);

		await keypad.enter("UNFREEZE", GROUP_KEYS, [], "UNFREEZE G10");
		await expect.poll(async () => freezeTargets(api)).toEqual({});
		for (const name of [
			"Dimmer 1",
			"Dimmer 2",
			"Dimmer 3",
			"Wash 101 · Master",
			"Wash 101 · zone 1",
			"Par 201",
		])
			await expect(sheet.status(name), name).toHaveCount(0);
		// The current underlying state is visible on the very next frame: Blackout is on.
		expect(slots(await outputFrame(bench, 0), DIMMER_SLOTS)).toEqual([0, 0, 0]);
		// No captured value was written into the Programmer or the Cue.
		const programmer = await programmerIntensities(api);
		expect(programmer[rig.dimmers[0]]).toBeCloseTo(0.1, 5);
		expect(programmer[rig.dimmers[1]]).toBeUndefined();
		expect(await cueListBody(api, rig.showId, rig.cueListId)).toEqual(cueList);
		// With Blackout off, the underlying 10 % x 30 % x 40 % shows instead of the captured 60 %.
		await setGlobalMaster(api, { blackout: false });
		expectDmxNear(slots(await outputFrame(bench, 0), [1])[0], 255 * 0.1 * 0.3 * 0.4);
	});

	test("FREEZE-FULL-002 @api › full Freeze of a multi-head fixture holds its Master-owned Pan, Tilt and Intensity output", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		const rig = await arrangeFreezeRig({ api, bench, desk, page, show });
		const captured = slots(await outputFrame(bench, 0), WASH_MASTER_SLOTS);
		await fullFreezeGroup(api);
		expect(
			slots(await frameAfterSourcesChange(api, bench, rig), WASH_MASTER_SLOTS),
		).toEqual(captured);
	});

	test("FREEZE-FULL-003 @api › full Freeze holds the physical colour of semantic-colour heads and fixtures", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		const rig = await arrangeFreezeRig({ api, bench, desk, page, show });
		const colored = [...rig.washHeads, rig.par];
		const colorSlots = [...WASH_HEAD_COLOR_SLOTS, ...PAR_SLOTS];
		await fullFreezeGroup(api);
		const frozen = slots(await outputFrame(bench, 0), colorSlots);
		const frozenColor = await visualizationValues(api, colored, ["color"]);
		const changed = await frameAfterSourcesChange(api, bench, rig);
		// The semantic (visualization) colour is held ...
		expect(await visualizationValues(api, colored, ["color"])).toEqual(
			frozenColor,
		);
		// ... and so must be the physical colour mix.
		expect(slots(changed, colorSlots)).toEqual(frozen);
	});

	test("FREEZE-FULL-004 @api › applying a full Freeze does not change the output it captures", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		test.fail(
			true,
			"BUG: Freeze on a virtual-dimmer colour fixture captures its visualization luminance as Intensity, so the ROOT PAR output drops at the Freeze (U1.202 204 -> 146 for 80 % green)",
		);
		await arrangeFreezeRig({ api, bench, desk, page, show });
		const before = slots(await outputFrame(bench, 0), PAR_SLOTS);
		await fullFreezeGroup(api);
		expect(slots(await outputFrame(bench, 0), PAR_SLOTS)).toEqual(before);
	});
	test("FREEZE-PARTIAL-001 @ui › keypad Intensity + Color Freeze retains only those families and follows Group Master, Grand Master and Blackout", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		test.setTimeout(90_000);
		const rig = await arrangeFreezeRig({ api, bench, desk, page, show });
		await desk.open(bench.baseUrl);
		const sheet = new FixtureSheetFreeze(page);
		await sheet.open();
		const keypad = new FreezeKeypad(page, desk);
		await keypad.enter(
			"FREEZE",
			GROUP_KEYS,
			["1", "2"],
			"FREEZE G10 INTENSITY COLOR",
		);

		await expect
			.poll(async () => Object.keys(await freezeTargets(api)).length)
			// Three dimmers, the wash root (its Master channels) and its three heads, the PAR.
			.toBe(3 + 1 + 3 + 1);
		for (const target of Object.values(await freezeTargets(api)))
			expect(target).toEqual({ full: false, families: ["color", "intensity"] });
		for (const name of ["Dimmer 1", "Wash 101 · zone 1", "Par 201"])
			await expect(sheet.status(name), name).toHaveText(
				"❄ FREEZE · Intensity · Color",
			);
		await expect(sheet.status("Dimmer 1")).not.toHaveText("❄ FREEZE");

		const frozenFrame = await outputFrame(bench, 0);
		const frozenSemantic = await visualizationValues(
			api,
			[...rig.dimmers, ...rig.washHeads, rig.par],
			["intensity", "color"],
		);
		const frozenPosition = slots(frozenFrame, [101, 102, 103, 104, 121]);

		// Change Intensity, Color, Position and Beam sources while the masters stay put.
		await changeSourcesOnly(api, bench, rig);
		const changed = await outputFrame(bench, 0);
		expect(
			await visualizationValues(
				api,
				[...rig.dimmers, ...rig.washHeads, rig.par],
				["intensity", "color"],
			),
			"Intensity and Color keep their captured semantic values",
		).toEqual(frozenSemantic);
		expect(slots(changed, DIMMER_SLOTS)).toEqual(
			slots(frozenFrame, DIMMER_SLOTS),
		);
		// Position (Pan/Tilt) and Beam (Zoom) are not frozen and follow the Programmer.
		const live = slots(changed, [101, 102, 103, 104, 121]);
		expect(live.slice(0, 4), "Position follows").not.toEqual(
			frozenPosition.slice(0, 4),
		);
		expect(live[4], "Beam follows").not.toBe(frozenPosition[4]);

		// The partial Freeze still follows all three masters, in proportion to their levels.
		const [heldOutput] = slots(changed, [1]);
		expect(heldOutput).toBeGreaterThan(20);
		await setGroupMaster(api, rig.groupMasterPlayback, 0.4);
		const [afterGroupMaster] = slots(await outputFrame(bench, 0), [1]);
		expectDmxNear(afterGroupMaster, heldOutput * (0.4 / 0.8));
		await setGlobalMaster(api, { grand_master: 0.45 });
		const [afterGrandMaster] = slots(await outputFrame(bench, 0), [1]);
		expectDmxNear(afterGrandMaster, heldOutput * (0.4 / 0.8) * (0.45 / 0.9));
		await setGlobalMaster(api, { blackout: true });
		expect(slots(await outputFrame(bench, 0), DIMMER_SLOTS)).toEqual([0, 0, 0]);
	});

	test("FREEZE-PARTIAL-002 @api › repeating the family action removes it, and a full Freeze over a partial one restores no partial metadata", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		const rig = await arrangeFreezeRig({ api, bench, desk, page, show });
		await api.executeCommandLine(`FREEZE GROUP ${FREEZE_GROUP} INTENSITY COLOR`);
		await expect
			.poll(async () => Object.keys(await freezeTargets(api)).length)
			.toBe(3 + 1 + 3 + 1);
		const held = slots(await outputFrame(bench, 0), [1]);
		await setIntensity(api, rig.showId, rig.dimmers[0], 0.1);
		expect(slots(await outputFrame(bench, 0), [1])).toEqual(held);

		// The same family action toggles those families, and their retained values, away.
		await toggleFamilies(api, ["intensity", "color"]);
		expect(await freezeTargets(api)).toEqual({});
		expectDmxNear(slots(await outputFrame(bench, 0), [1])[0], 255 * 0.1 * 0.8 * 0.9);

		await toggleFamilies(api, ["intensity"]);
		for (const target of Object.values(await freezeTargets(api)))
			expect(target).toEqual({ full: false, families: ["intensity"] });
		// The wash Master's Position is captured from an accepted frame after the patch change.
		await bench.tick(25);
		await fullFreezeGroup(api);
		for (const target of Object.values(await freezeTargets(api)))
			expect(target).toEqual({ full: true, families: [] });
		await api.executeCommandLine(`UNFREEZE GROUP ${FREEZE_GROUP}`);
		expect(
			await freezeTargets(api),
			"removing the full Freeze restores no partial-family metadata",
		).toEqual({});
	});
	test("FREEZE-PARTIAL-003 @api › a partial Intensity Freeze retains the pre-master semantic value, so the output does not change when it is applied", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		test.fail(
			true,
			"BUG: partial Freeze captures the post-master output as its semantic Intensity, so Group Master and Grand Master apply twice (Dimmer 1 drops 110 -> 79 at the Freeze: 60 % x 80 % x 90 % retained as 43 %)",
		);
		const rig = await arrangeFreezeRig({ api, bench, desk, page, show });
		const before = slots(await outputFrame(bench, 0), [1]);
		expectDmxNear(before[0], 255 * 0.6 * 0.8 * 0.9);
		await api.executeCommandLine(`FREEZE GROUP ${FREEZE_GROUP} INTENSITY`);
		expect(slots(await outputFrame(bench, 0), [1])).toEqual(before);
		expect(
			await visualizationValues(api, [rig.dimmers[0]], ["intensity"]),
		).toEqual({
			[`${rig.dimmers[0]}/intensity`]: { kind: "normalized", value: expect.closeTo(0.6, 5) },
		});
	});
	test("FREEZE-PERSISTENCE-001 @ui › saved and reopened shows keep full and partial Freeze, also after an unrelated Show Patch edit", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		test.setTimeout(90_000);
		await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
		const [dimmer1, dimmer2] = show.fixtureIds;
		await setIntensity(api, show.id, dimmer1, 0.6);
		await setIntensity(api, show.id, dimmer2, 0.6);
		await outputFrame(bench);
		await api.executeCommandLine("FREEZE 1");
		await api.executeCommandLine("FREEZE 2 INTENSITY");
		await setIntensity(api, show.id, dimmer1, 0.1);
		await setIntensity(api, show.id, dimmer2, 0.1);
		const stored = await freezeTargets(api);
		expect(stored).toEqual({
			[dimmer1]: { full: true, families: [] },
			[dimmer2]: { full: false, families: ["intensity"] },
		});
		const held = slots(await outputFrame(bench), [1, 2]);
		expect(held).toEqual([153, 153]);
		const retained = await visualizationValues(api, [dimmer1, dimmer2], ["intensity"]);

		const expectReopened = async (label: string) => {
			expect(await freezeTargets(api), `${label}: stored Freeze`).toEqual(stored);
			expect(slots(await outputFrame(bench), [1, 2]), `${label}: output`).toEqual(held);
			expect(
				await visualizationValues(api, [dimmer1, dimmer2], ["intensity"]),
				`${label}: retained values`,
			).toEqual(retained);
			await desk.open(bench.baseUrl);
			const sheet = new FixtureSheetFreeze(page);
			await sheet.open();
			await expect(sheet.status("Dimmer 1")).toHaveText("❄ FREEZE");
			await expect(sheet.status("Dimmer 2")).toHaveText("❄ FREEZE · Intensity");
			await expect(sheet.status("Dimmer 3")).toHaveCount(0);
		};

		// Save the show file, close it, and open the saved file.
		const saved = await api.createShow<{ id: string }>({
			name: `FREEZE-PERSISTENCE saved ${crypto.randomUUID()}`,
			data_base64: (await api.downloadShow(show.id)).toString("base64"),
		});
		await api.openShow(saved.id, { transition: "hold_current" });
		await expectReopened("saved file");

		// An unrelated Show Patch edit, then close and reopen again.
		await new BrowserPatch(api, page, desk).via.api.add({
			number: 201,
			name: "Par 201",
			manufacturer: "Cameo",
			profile: "ROOT PAR 6",
			mode: "D7CH — Delay Off, virtual dimmer",
			address: "1.201",
		});
		await api.openShow(show.id, { transition: "hold_current" });
		await api.openShow(saved.id, { transition: "hold_current" });
		await expectReopened("after an unrelated Patch edit");
	});

	test("FREEZE-PERSISTENCE-002 @ui › FREEZE/UNFREEZE from the touch keypad, OSC and attached hardware reach the same live action and one Patch transaction each", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		test.setTimeout(90_000);
		await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
		const dimmer1 = show.fixtureIds[0];
		await desk.open(bench.baseUrl);
		const keypad = new FreezeKeypad(page, desk);
		const osc = await bench.osc();
		const oscClient = `freeze-osc-${crypto.randomUUID()}`;
		const hardware = simulatedHardware(bench, api);
		const oscKeys = new FreezeOscKeys(osc, "desk");
		let hardwareKeys: FreezeOscKeys | undefined;
		// The software keypad leaves the screen once a controller is attached, so the touch path
		// runs first and the OSC paths connect afterwards.
		const connect = async () => {
			if (hardwareKeys) return;
			await osc.subscribe(oscClient, "desk");
			hardwareKeys = new FreezeOscKeys(await hardware.connect(), "desk");
		};
		const held = () => {
			if (!hardwareKeys) throw new Error("attached hardware is not connected");
			return hardwareKeys;
		};

		const surfaces: Record<string, { freeze(): Promise<void>; unfreeze(): Promise<void> }> = {
			"touch keypad": {
				freeze: () => keypad.enter("FREEZE", ["1"]),
				unfreeze: () => keypad.enter("UNFREEZE", ["1"]),
			},
			"OSC (Shift chord per key)": {
				freeze: async () => {
					await connect();
					await oscKeys.keys("shift", "clear");
					await oscKeys.key("shift", false);
					await expect.poll(() => commandLineText(api)).toBe("FREEZE");
					await oscKeys.key("digit-1");
					await expect.poll(() => commandLineText(api)).toMatch(/^(?:UN)?FREEZE 1$/);
					await oscKeys.key("enter");
				},
				unfreeze: async () => {
					for (const _ of [1, 2]) {
						await oscKeys.keys("shift", "clear");
						await oscKeys.key("shift", false);
					}
					await expect.poll(() => commandLineText(api)).toBe("UNFREEZE");
					await oscKeys.key("digit-1");
					await expect.poll(() => commandLineText(api)).toMatch(/^(?:UN)?FREEZE 1$/);
					await oscKeys.key("enter");
				},
			},
			"attached hardware (Shift held over both Clears)": {
				freeze: async () => {
					await held().keys("shift", "clear");
					await held().key("shift", false);
					await expect.poll(() => commandLineText(api)).toBe("FREEZE");
					await held().key("digit-1");
					await expect.poll(() => commandLineText(api)).toMatch(/^(?:UN)?FREEZE 1$/);
					await held().key("enter");
				},
				unfreeze: async () => {
					await held().keys("shift", "clear", "clear");
					await held().key("shift", false);
					await expect.poll(() => commandLineText(api)).toBe("UNFREEZE");
					await held().key("digit-1");
					await expect.poll(() => commandLineText(api)).toMatch(/^(?:UN)?FREEZE 1$/);
					await held().key("enter");
				},
			},
		};
		try {
			for (const [surface, path] of Object.entries(surfaces)) {
				await setIntensity(api, show.id, dimmer1, 0.6);
				await outputFrame(bench);
				const before = await patchRevision(api);
				await path.freeze();
				await expect
					.poll(() => freezeTargets(api), { message: `${surface}: FREEZE` })
					.toEqual({ [dimmer1]: { full: true, families: [] } });
				expect(await patchRevision(api), `${surface}: one Patch transaction`).toBe(before + 1);
				await expect.poll(() => commandLineText(api)).toMatch(/^(?:FIXTURE|GROUP)$/);
				await setIntensity(api, show.id, dimmer1, 0.1);
				expect(slots(await outputFrame(bench), [1]), `${surface}: held output`).toEqual([153]);

				await path.unfreeze();
				await expect
					.poll(() => freezeTargets(api), { message: `${surface}: UNFREEZE` })
					.toEqual({});
				expect(await patchRevision(api), `${surface}: one Patch transaction`).toBe(before + 2);
				await expect.poll(() => commandLineText(api)).toMatch(/^(?:FIXTURE|GROUP)$/);
				expect(slots(await outputFrame(bench), [1]), `${surface}: live output`).toEqual([26]);
			}
		} finally {
			await hardware.disconnect().catch(() => undefined);
			await osc.unsubscribe(oscClient).catch(() => undefined);
		}
	});

	test("FREEZE-PERSISTENCE-003 @ui › UND reverses a newer Programmer edit first, then restores the exact pre-Freeze state, with no Redo", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
		const [dimmer1, dimmer2] = show.fixtureIds;
		await api.executeCommandLine("1 AT 40");
		expect(slots(await outputFrame(bench), [1])).toEqual([102]);
		await api.executeCommandLine("FREEZE 1");
		await api.executeCommandLine("2 AT 70");
		expect(slots(await outputFrame(bench), [1, 2])).toEqual([102, 179]);

		await desk.open(bench.baseUrl);
		const keypad = new FreezeKeypad(page, desk);
		await keypad.undo();
		await expect.poll(async () => (await programmerIntensities(api))[dimmer2]).toBeUndefined();
		expect(await freezeTargets(api), "the first Undo keeps the Freeze").toEqual({
			[dimmer1]: { full: true, families: [] },
		});

		await keypad.undo();
		await expect.poll(() => freezeTargets(api)).toEqual({});
		const programmer = await programmerIntensities(api);
		expect(programmer[dimmer1], "the pre-Freeze Programmer value").toBeCloseTo(0.4, 5);
		expect(programmer[dimmer2]).toBeUndefined();
		expect(slots(await outputFrame(bench), [1, 2])).toEqual([102, 0]);

		// Freeze created no Redo: further Undo walks the ordinary Programmer history (the FREEZE
		// command's selection, then "1 AT 40") and never re-applies the Freeze.
		for (const _ of [1, 2]) {
			const revision = await patchRevision(api);
			await keypad.undo();
			await expect
				.poll(() => api.request<{ projection: { revision: number } }>("GET", "/api/v2/programmer/values/snapshot"))
				.toBeTruthy();
			expect(await freezeTargets(api)).toEqual({});
			expect(await patchRevision(api)).toBe(revision);
		}
	});

	test("FREEZE-PERSISTENCE-004 @api › an older show without Freeze fields opens unfrozen and saves without recovery warnings", async ({
		api,
		bench,
	}) => {
		const older = await loadCanonicalCopy(api, bench, "freeze-older-show", "compact-rig");
		const readiness = async () =>
			api.request<{ active_show: string | null; active_show_error: string | null; recovery_mode: boolean }>(
				"GET",
				"/api/v2/readiness",
				undefined,
				false,
			);
		expect(await readiness()).toMatchObject({
			active_show: older.id,
			active_show_error: null,
			recovery_mode: false,
		});
		expect(await freezeTargets(api)).toEqual({});
		const fixtures = await patchFixtures(api);
		expect(fixtures.length).toBeGreaterThan(0);
		for (const fixture of fixtures) expect(fixture.freeze_targets ?? []).toEqual([]);

		await api.saveShowRevision(older.id, "freeze older show save");
		const reopened = await api.createShow<{ id: string }>({
			name: `freeze-older-show-saved-${crypto.randomUUID()}`,
			data_base64: (await api.downloadShow(older.id)).toString("base64"),
		});
		await api.openShow(reopened.id, { transition: "hold_current" });
		expect(await readiness()).toMatchObject({
			active_show: reopened.id,
			active_show_error: null,
			recovery_mode: false,
		});
		expect(await freezeTargets(api)).toEqual({});
	});
});
