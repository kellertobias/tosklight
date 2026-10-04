import type { Locator } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import type { PsnStream } from "./bench/protocols/psnSender";
import {
	aimAtPoint,
	artNetDmxPacket,
	createMacro,
	freePsnPort,
	moverSlots,
	occupyUdpPort,
	openPsnStream,
	openTrackingTab,
	outputFrame,
	patchTrackingRig,
	pointAxisValue,
	pointWorldPosition,
	readPsn,
	refusedPsnEdit,
	releaseFixtureValue,
	reopenFromSavedFile,
	semanticPositionPublished,
	setFixtureValue,
	showPatchHeader,
	type TrackingRig,
	trackerAt,
	trackingMacroRuns,
	updatePsn,
} from "./bench/tracking/psnTracking";
import { loadCanonicalCopy } from "./support/catalog";

/**
 * docs/testing/19-tracking-with-posistagenet.md: a PosiStageNet stream from the bench sender
 * (tests/bench/protocols/psnSender.ts, byte-for-byte the Rust encoder's format) drives the desk's
 * real receiver. Every case configures the receiver on its own free UDP port and sends unicast to
 * 127.0.0.1; the receiver binds the wildcard address on that port and joins the group, so this is
 * the same socket path multicast traffic takes, without depending on a multicast route in CI.
 *
 * PSN arrival and the receiver's 20 ms tick run on real time; DMX output and Point poses are read
 * after advancing the bench's manual output clock.
 */

const GATE =
	"semantic programming contract is not enabled on this runtime (Position Target at a 3D Point is a semantic Position value)";

type Vec3 = [number, number, number];

function near(actual: readonly number[] | null | undefined, expected: readonly number[], tolerance = 0.01) {
	return (
		actual != null &&
		actual.length === expected.length &&
		actual.every((value, axis) => Math.abs(value - expected[axis]) <= tolerance)
	);
}

const pause = (millis: number) => new Promise((resolve) => setTimeout(resolve, millis));

/** Advance one output frame and return where it puts the Point. */
async function framePoint(api: ApiDriver, bench: LightBench, rig: TrackingRig) {
	await bench.tick(25);
	return pointWorldPosition(api, rig);
}

async function expectPointAt(api: ApiDriver, bench: LightBench, rig: TrackingRig, expected: Vec3) {
	await expect
		.poll(async () => near(await framePoint(api, bench, rig), expected), {
			message: `the Point reaches ${expected.join(", ")} m`,
			timeout: 5_000,
		})
		.toBe(true);
}

async function trackerPosition(api: ApiDriver, trackerId: number) {
	const psn = await readPsn(api);
	return (
		psn.status.trackers.find((tracker) => tracker.tracker_id === trackerId && !tracker.stale)?.position_metres ?? null
	);
}

async function expectTrackerAt(api: ApiDriver, trackerId: number, expected: Vec3) {
	await expect
		.poll(async () => near(await trackerPosition(api, trackerId), expected), {
			message: `tracker ${trackerId} is reported at ${expected.join(", ")} m`,
			timeout: 5_000,
		})
		.toBe(true);
}

/** Receive PSN on a fresh port with the given stale timeout, and open a stream towards it. */
async function listening(api: ApiDriver, staleAfterMillis = 1_000) {
	const port = await freePsnPort();
	await updatePsn(api, { enabled: true, port, stale_after_millis: staleAfterMillis });
	const stream = await openPsnStream(port);
	return { port, stream };
}

function bind(rig: TrackingRig, trackerId = 1, enabled = true) {
	return { id: crypto.randomUUID(), tracker_id: trackerId, point_fixture_id: rig.pointId, enabled };
}

/** Touch a switch where the operator does: its visible track. */
async function toggle(control: Locator) {
	await control.locator("..").locator(".ui-switch-track").click();
}

async function universeOne(api: ApiDriver, bench: LightBench) {
	await bench.tick(25);
	return (await outputFrame(api)).universes.find((universe) => universe.universe === 1)?.slots ?? [];
}

test.describe("docs/testing/19-tracking-with-posistagenet.md", () => {
	let streams: PsnStream[] = [];
	test.beforeEach(() => {
		streams = [];
	});
	test.afterEach(async () => {
		await Promise.all(streams.map((stream) => stream.close()));
	});
	const track = (stream: PsnStream) => {
		streams.push(stream);
		return stream;
	};

	test("PSN-NOTHING-BOUND @ui › traffic alone lists trackers in show metres and moves nothing", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		const rig = await patchTrackingRig(api, show.id);
		const port = await freePsnPort();
		// Where the desk listens is a Settings value (PSN-SETTINGS-IN-REACH); this case uses only the
		// tab's switch, so the bench's own port is stored first while tracking stays off.
		await updatePsn(api, { port });
		const tab = await openTrackingTab(page, desk, api.baseUrl);
		await expect(tab.locator(".psn-status")).toHaveText("Not listening. Tracking is switched off.");
		await expect(tab).toContainText("Nothing is bound.");
		await expect(tab.getByRole("alert")).toHaveCount(0);

		await bench.tick(25);
		const before = await outputFrame(api);
		const programmerRevision = async () =>
			(await api.request<{ projection: { revision: number } }>("GET", "/api/v2/programmer/values/snapshot"))
				.projection.revision;
		const programmerBefore = await programmerRevision();

		await toggle(tab.getByRole("switch", { name: /Receive PosiStageNet/ }));
		await expect(tab.locator(".psn-status")).toContainText(`Listening on 236.10.10.10:${port}`);
		const stream = track(await openPsnStream(port, "127.0.0.1", "Bench tracking server"));
		stream.update(
			[trackerAt(1, [1, 2, -3]), trackerAt(2, [-1, 0.5, 2])],
			[
				{ id: 1, name: "Presenter" },
				{ id: 2, name: "Guitar" },
			],
		);
		stream.start();

		const rows = tab.locator("table.psn-trackers tbody tr");
		await expect(rows).toHaveCount(2);
		await expect(rows.nth(0)).toContainText("1 · Presenter");
		await expect(rows.nth(0)).toContainText("1.00m, 3.00m, 2.00m");
		await expect(rows.nth(0)).toContainText(/\d+ms ago/);
		await expect(rows.nth(1)).toContainText("2 · Guitar");
		await expect(rows.nth(1)).toContainText("-1.00m, -2.00m, 0.50m");
		await expect(tab.locator(".psn-status")).toContainText(
			`Receiving on 236.10.10.10:${port} from Bench tracking server — 2 tracker(s)`,
		);
		await expect(tab).toContainText("Nothing is bound.");

		// Traffic alone moves nothing: no Point pose, no DMX slot and no Programmer value changed.
		for (let frame = 0; frame < 4; frame += 1) await bench.tick(25);
		const after = await outputFrame(api);
		expect(after.universes).toEqual(before.universes);
		expect(after.points.find((point) => point.fixture_id === rig.pointId)?.offset_metres).toEqual(
			before.points.find((point) => point.fixture_id === rig.pointId)?.offset_metres,
		);
		expect(await programmerRevision()).toBe(programmerBefore);
		expect((await readPsn(api)).status.placements).toEqual([]);
	});

	test("PSN-COORDINATE-BOUNDARY @api › PSN x/y/z becomes desk x/-z/y, Y is height and rotation turns about desk Z", async ({
		api,
		bench,
		show,
	}) => {
		const rig = await patchTrackingRig(api, show.id);
		const { stream } = await listening(api);
		track(stream);
		await updatePsn(api, { bindings: [bind(rig)] });

		// 1. Zero calibration: PSN (1, 2, -3) is desk (1, 3, 2).
		stream.update([trackerAt(1, [1, 2, -3])]);
		stream.start();
		await expectTrackerAt(api, 1, [1, 3, 2]);
		await expectPointAt(api, bench, rig, [1, 3, 2]);

		// 2. Only PSN Y rises: the Point rises in desk Z, neither across nor up/downstage.
		stream.update([trackerAt(1, [1, 3.5, -3])]);
		await expectTrackerAt(api, 1, [1, 3, 3.5]);
		await expectPointAt(api, bench, rig, [1, 3, 3.5]);

		// 3. 90° calibration rotation turns the Point in the desk XY plane; height is unchanged.
		stream.update([trackerAt(1, [1, 2, -3])]);
		await updatePsn(api, { calibration: { offset_metres: [0, 0, 0], rotation_degrees: 90, scale: 1 } });
		await expectTrackerAt(api, 1, [-3, 1, 2]);
		await expectPointAt(api, bench, rig, [-3, 1, 2]);
		const placement = (await readPsn(api)).status.placements[0];
		expect(near(placement?.position_metres, [-3, 1, 2])).toBe(true);
		expect(placement?.out_of_reach).toBe(false);
	});

	test("PSN-MARKER-MOVES-POINT @ui › a bound marker owns the Point against cues and encoders, and the aimed light follows", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		// 1. A 3D Point and a mover aimed at it (Position Target with the Point as reference).
		const rig = await patchTrackingRig(api, show.id);
		requireSemanticContract(await semanticPositionPublished(api, [rig.moverId]), GATE);
		await aimAtPoint(api, [rig.moverId], rig.pointId);
		const { stream } = await listening(api);
		track(stream);
		stream.update([trackerAt(1, [2, 1, -3])], [{ id: 1, name: "Presenter" }]);
		stream.start();
		await bench.tick(25);
		const restingAim = await moverSlots(api, rig);

		// Bind tracker 1 on the Tracking tab.
		const tab = await openTrackingTab(page, desk, api.baseUrl);
		await expect(tab.locator("table.psn-trackers tbody tr")).toContainText("1 · Presenter");
		await tab.getByLabel("3D Point for tracker 1").click();
		await page.getByRole("option", { name: "901 · Presenter point", exact: true }).click();
		await expect(tab.locator(".psn-bindings")).toContainText("Tracker 1 → Presenter point");
		await expect(tab.locator(".psn-placement")).toHaveText("2.00m, 3.00m, 1.00m");

		// 2. The Point follows the marker, and the light aimed at the Point follows the Point.
		await expectPointAt(api, bench, rig, [2, 3, 1]);
		const aimA = await moverSlots(api, rig);
		expect(aimA.slice(0, 4)).not.toEqual(restingAim.slice(0, 4));
		stream.update([trackerAt(1, [-2, 1, -3])]);
		await expectPointAt(api, bench, rig, [-2, 3, 1]);
		const aimB = await moverSlots(api, rig);
		expect(aimB.slice(0, 4), "Pan/Tilt follow the Point across the stage").not.toEqual(aimA.slice(0, 4));
		await expect(tab.locator(".psn-placement")).toHaveText("-2.00m, 3.00m, 1.00m");

		// 3. A cue that stores a position for the Point cannot pull it away from the marker.
		await setFixtureValue(api, rig.pointId, "point.position.x", pointAxisValue(5));
		await api.executeCommandLine("RECORD PBK 1");
		await releaseFixtureValue(api, rig.pointId, "point.position.x");
		stream.update([trackerAt(1, [-1, 1, -3])]);
		await api.executeCommandLine("GO TO PBK 1 CUE 1");
		await expectPointAt(api, bench, rig, [-1, 3, 1]);
		for (let frame = 0; frame < 4; frame += 1)
			expect(near(await framePoint(api, bench, rig), [-1, 3, 1]), "the cue does not take the Point").toBe(true);

		// 4. Nor can the Point's position encoder.
		await setFixtureValue(api, rig.pointId, "point.position.x", pointAxisValue(7));
		stream.update([trackerAt(1, [1, 1, -3])]);
		await expectPointAt(api, bench, rig, [1, 3, 1]);
		for (let frame = 0; frame < 4; frame += 1)
			expect(near(await framePoint(api, bench, rig), [1, 3, 1]), "the encoder does not take the Point").toBe(true);

		// 5. Switching the binding off returns the Point to the show (the Programmer's X +7 m over
		// the cue's) in the next output frame; the tracker stays listed and moving.
		const bindingSwitch = tab.getByRole("switch", { name: /Binding 1/ });
		await toggle(bindingSwitch);
		await expect(bindingSwitch).not.toBeChecked();
		expect(near(await framePoint(api, bench, rig), [7, 4, 0])).toBe(true);
		stream.update([trackerAt(1, [3, 1, -3])]);
		await expectTrackerAt(api, 1, [3, 3, 1]);
		await expect(tab.locator("table.psn-trackers tbody tr")).toContainText("3.00m, 3.00m, 1.00m");
		expect(near(await framePoint(api, bench, rig), [7, 4, 0])).toBe(true);
		// Releasing the encoder value shows the cue did store a position: the Point goes to X +5 m.
		await releaseFixtureValue(api, rig.pointId, "point.position.x");
		await expectPointAt(api, bench, rig, [5, 4, 0]);
	});

	test("PSN-SILENCE-HOLDS @ui › a silent source is stale and holds, resumes by itself, and Receive off returns the show", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		const rig = await patchTrackingRig(api, show.id);
		const semantic = await semanticPositionPublished(api, [rig.moverId]);
		if (semantic) await aimAtPoint(api, [rig.moverId], rig.pointId);
		const { port, stream } = await listening(api, 400);
		track(stream);
		await updatePsn(api, { bindings: [bind(rig)] });
		stream.update([trackerAt(1, [2, 1, -3])], [{ id: 1, name: "Presenter" }]);
		stream.start();
		await expectPointAt(api, bench, rig, [2, 3, 1]);
		const held = await moverSlots(api, rig);
		const tab = await openTrackingTab(page, desk, api.baseUrl);
		await expect(tab.locator(".psn-status")).toContainText(`Receiving on 236.10.10.10:${port}`);

		// 1–2. The sender stops: the source and its tracker are reported stale, with the silence.
		await stream.stop();
		await expect(tab.locator(".psn-status")).toContainText(
			new RegExp(
				`Nothing heard on 236\\.10\\.10\\.10:${port} for \\d+s\\. Bound points are holding their last position\\.`,
			),
		);
		const row = tab.locator("table.psn-trackers tbody tr").first();
		await expect(row).toHaveClass(/is-stale/);
		await expect(row).toContainText(/\d+s ago — stale/);
		const status = (await readPsn(api)).status;
		expect(status.health).toMatchObject({ state: "stale" });
		expect(status.trackers[0]?.stale).toBe(true);

		// 3. The Point holds the last position that arrived, and so does the light.
		for (let frame = 0; frame < 4; frame += 1) expect(near(await framePoint(api, bench, rig), [2, 3, 1])).toBe(true);
		expect(await moverSlots(api, rig)).toEqual(held);

		// 4. The same sender starts again: the Point picks the marker up with no operator action.
		stream.update([trackerAt(1, [-2, 1, -3])]);
		stream.start();
		await expectPointAt(api, bench, rig, [-2, 3, 1]);
		await expect(tab.locator(".psn-status")).toContainText(`Receiving on 236.10.10.10:${port}`);
		if (semantic) expect(await moverSlots(api, rig)).not.toEqual(held);

		// 5. Receive off: every bound Point returns to the show at once; the binding stays listed.
		await toggle(tab.getByRole("switch", { name: /Receive PosiStageNet/ }));
		await expect(tab.locator(".psn-status")).toHaveText("Not listening. Tracking is switched off.");
		expect(near(await framePoint(api, bench, rig), rig.pointOrigin)).toBe(true);
		await expect(tab.locator(".psn-bindings")).toContainText("Tracker 1 → Presenter point");
		expect((await readPsn(api)).configuration.bindings).toHaveLength(1);
	});

	test("PSN-SILENCE-HOLDS-NEW-SOURCE @api › a sender restarted from a new source port reads as receiving again", async ({
		api,
		bench,
		show,
	}) => {
		test.fail(
			true,
			"BUG: after a PSN sender restarts from a new UDP source port, status.health stays `stale` (worst of all sources; the silent old source is never expired) although frames arrive",
		);
		const rig = await patchTrackingRig(api, show.id);
		const { port, stream } = await listening(api, 300);
		track(stream);
		await updatePsn(api, { bindings: [bind(rig)] });
		stream.update([trackerAt(1, [2, 1, -3])]);
		stream.start();
		await expectPointAt(api, bench, rig, [2, 3, 1]);
		await stream.stop();
		await expect.poll(async () => (await readPsn(api)).status.health?.state).toBe("stale");

		// A tracking server restarted on its machine sends from a new ephemeral port.
		const restarted = track(await openPsnStream(port));
		restarted.update([trackerAt(1, [-2, 1, -3])]);
		restarted.start();
		// The Point is picked up (this part works) …
		await expectPointAt(api, bench, rig, [-2, 3, 1]);
		// … but the status keeps saying nothing has been heard.
		await expect.poll(async () => (await readPsn(api)).status.health?.state, { timeout: 3_000 }).toBe("receiving");
	});

	test("PSN-ZONES-RUN-MACROS @api › a zone runs its Macros once per crossing, ignores boundary jitter and silence", async ({
		api,
		bench,
		show,
	}) => {
		// The command line has no text form for a playback Off, so the two Macros switch the front
		// dimmers on and off; what is under test is how often the zone runs them.
		const enter = await createMacro(api, show.id, 41, "Front wash on", "GROUP 3 AT 100");
		const leave = await createMacro(api, show.id, 42, "Front wash off", "GROUP 3 AT 0");
		const { stream } = await listening(api, 300);
		track(stream);
		const zoneId = crypto.randomUUID();
		// Downstage box in desk metres: across ±2, from 6 m to 1 m downstage, up to 3 m. Hold for
		// stays at its 250 ms default.
		await updatePsn(api, {
			zones: [
				{
					id: zoneId,
					name: "Downstage",
					min_metres: [-2, -6, -1],
					max_metres: [2, -1, 3],
					tracker_ids: [],
					enter_macro_id: enter,
					leave_macro_id: leave,
					dwell_millis: 250,
				},
			],
		});
		const runs = () => trackingMacroRuns(api, show.id);
		const occupied = async () => (await readPsn(api)).status.occupied_zone_ids.includes(zoneId);
		const frontDimmers = async () => (await universeOne(api, bench)).slice(0, 4);

		// Upstage, outside: PSN depth -3 m is desk y +3 m.
		stream.update([trackerAt(1, [0, 1, -3])]);
		stream.start();
		await expect.poll(async () => (await readPsn(api)).status.trackers.length).toBe(1);
		await pause(400);
		expect(await runs()).toEqual({});
		expect(await occupied()).toBe(false);

		// 2. Walk in (desk y -3): the entering Macro runs once and the zone is occupied.
		stream.update([trackerAt(1, [0, 1, 3])]);
		await expect.poll(occupied).toBe(true);
		await expect.poll(runs).toEqual({ [enter]: 1 });
		await expect.poll(frontDimmers).toEqual([255, 255, 255, 255]);

		// 3. On the boundary (desk y -1): the reported position crosses in and out every frame.
		stream.cycle([[trackerAt(1, [0, 1, 1.05])], [trackerAt(1, [0, 1, 0.95])]]);
		await pause(1_200);
		expect(await runs()).toEqual({ [enter]: 1 });
		expect(await occupied()).toBe(true);

		// 4. Walk out: the leaving Macro runs once.
		stream.update([trackerAt(1, [0, 1, -3])]);
		await expect.poll(occupied).toBe(false);
		await expect.poll(runs).toEqual({ [enter]: 1, [leave]: 1 });
		await expect.poll(frontDimmers).toEqual([0, 0, 0, 0]);
		await pause(500);
		expect(await runs()).toEqual({ [enter]: 1, [leave]: 1 });

		// 5. Walk in again, then the sender stops: no leaving Macro, the zone stays occupied.
		stream.update([trackerAt(1, [0, 1, 3])]);
		await expect.poll(runs).toEqual({ [enter]: 2, [leave]: 1 });
		await stream.stop();
		await expect.poll(async () => (await readPsn(api)).status.health?.state).toBe("stale");
		await pause(800);
		expect(await runs()).toEqual({ [enter]: 2, [leave]: 1 });
		expect(await occupied()).toBe(true);
	});

	test("PSN-UNWANTED-ART-NET @api › an Art-Net packet on the PSN port is counted as ignored and PSN keeps arriving", async ({
		api,
	}) => {
		const { stream } = await listening(api);
		track(stream);
		stream.update([trackerAt(1, [0, 1, 0])]);
		stream.start();
		await expect.poll(async () => (await readPsn(api)).status.frames).toBeGreaterThan(0);
		expect((await readPsn(api)).status.ignored_datagrams).toBe(0);

		await stream.sender.sendRaw(artNetDmxPacket(0, new Uint8Array(512).fill(255)));
		await expect.poll(async () => (await readPsn(api)).status.ignored_datagrams).toBe(1);
		const after = (await readPsn(api)).status;
		expect(after.trackers.map((tracker) => tracker.tracker_id)).toEqual([1]);
		await expect.poll(async () => (await readPsn(api)).status.frames).toBeGreaterThan(after.frames);
		expect((await readPsn(api)).status.health).toMatchObject({ state: "receiving" });
	});

	test("PSN-UNWANTED-UNICAST-GROUP @api › a group that is not multicast is refused, named, and the desk keeps listening", async ({
		api,
	}) => {
		const { port, stream } = await listening(api);
		track(stream);
		stream.update([trackerAt(1, [0, 1, 0])]);
		stream.start();
		await expect.poll(async () => (await readPsn(api)).status.health?.state).toBe("receiving");
		const before = await readPsn(api);

		const refusal = await refusedPsnEdit(api, { group: "192.168.1.20" });
		expect(refusal).toMatch(/returned 400/);
		expect(refusal).toContain("192.168.1.20 is not a multicast group; PosiStageNet transmits to one");
		const after = await readPsn(api);
		expect(after.revision).toBe(before.revision);
		expect(after.configuration).toEqual(before.configuration);
		expect(after.status.listening_on).toBe(`236.10.10.10:${port}`);
		const frames = after.status.frames;
		await expect.poll(async () => (await readPsn(api)).status.frames).toBeGreaterThan(frames);
	});

	test("PSN-UNWANTED-PORT-IN-USE @ui › a port held by another program is an actionable error and the desk keeps working", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const occupied = await occupyUdpPort();
		try {
			await updatePsn(api, { enabled: true, port: occupied.port });
			const tab = await openTrackingTab(page, desk, api.baseUrl);
			const alert = tab.getByRole("alert");
			await expect(alert).toContainText(`the desk could not listen for PSN on 236.10.10.10:${occupied.port}`);
			await expect(alert).toContainText(/in use/i);

			// The rest of the desk keeps working: a command still reaches the output.
			await api.executeCommandLine("GROUP 3 AT 100");
			await expect.poll(async () => (await universeOne(api, bench)).slice(0, 4)).toEqual([255, 255, 255, 255]);

			// Moving to a free port clears the error.
			await updatePsn(api, { port: await freePsnPort() });
			await expect(alert).toHaveCount(0);
		} finally {
			await occupied.release();
		}
	});

	test("PSN-SETTINGS-IN-REACH @ui › Show Patch tabs stay put and Tracking Settings keep, validate and report the source", async ({
		api,
		desk,
		page,
	}) => {
		await page.setViewportSize({ width: 1280, height: 560 });
		await updatePsn(api, { enabled: true, port: await freePsnPort() });
		await openTrackingTab(page, desk, api.baseUrl);
		const header = showPatchHeader(page);

		// 1. The tabs and ⚙ stay in exactly the same place; Tracking scrolls to its last control.
		const places = async () => {
			const boxes: Record<string, unknown> = {};
			for (const name of ["Fixtures", "Media Servers", "Tracking"])
				boxes[name] = await header.getByRole("tab", { name, exact: true }).boundingBox();
			boxes.Settings = await header.getByRole("button", { name: "Settings", exact: true }).boundingBox();
			return JSON.stringify(boxes, (_key, value) => (typeof value === "number" ? Math.round(value) : value));
		};
		const onTracking = await places();
		for (const name of ["Fixtures", "Media Servers", "Tracking"]) {
			await header.getByRole("tab", { name, exact: true }).click();
			expect(await places()).toBe(onTracking);
		}
		const scroller = page.locator(".patch-configuration-window .patch-configuration-scroll .ui-window-scroller");
		await scroller.evaluate((node) => {
			node.scrollTop = node.scrollHeight;
		});
		await expect(page.locator(".psn-setup > :last-child")).toBeInViewport();
		await expect(page.locator(".psn-setup").getByLabel("Multicast group")).toHaveCount(0);

		// 2. ⚙ opens Settings on Tracking with the stored values.
		const stored = (await readPsn(api)).configuration;
		await header.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Show Patch" });
		await expect(settings.getByRole("tab", { name: "Tracking", exact: true })).toHaveAttribute("aria-selected", "true");
		const group = settings.getByLabel("Multicast group");
		const port = settings.getByLabel("Port", { exact: true });
		const stale = settings.getByLabel("Stale after (ms)");
		await expect(group).toHaveValue(stored.group);
		await expect(port).toHaveValue(String(stored.port));
		await expect(stale).toHaveValue(String(stored.stale_after_millis));

		// 3. A group outside 224.0.0.0–239.255.255.255 is named and nothing is stored.
		const apply = settings.getByRole("button", { name: "Apply tracking settings" });
		await group.fill("10.0.0.1");
		await apply.scrollIntoViewIfNeeded();
		await apply.click();
		await expect(settings).toContainText("use 224.0.0.0 to 239.255.255.255");
		expect((await readPsn(api)).configuration).toEqual(stored);

		// 4. Valid values are stored, come back on reopen, and the desk reports them.
		const nextPort = await freePsnPort();
		await group.fill("239.1.2.3");
		await port.fill(String(nextPort));
		await stale.fill("2500");
		await apply.scrollIntoViewIfNeeded();
		await apply.click();
		await expect(settings.getByRole("status")).toHaveText("Tracking settings saved.");
		await settings.getByRole("button", { name: "Close settings" }).click();
		await header.getByRole("button", { name: "Settings", exact: true }).click();
		await expect(group).toHaveValue("239.1.2.3");
		await expect(port).toHaveValue(String(nextPort));
		await expect(stale).toHaveValue("2500");
		expect((await readPsn(api)).configuration).toMatchObject({
			group: "239.1.2.3",
			port: nextPort,
			stale_after_millis: 2500,
		});
		await expect.poll(async () => (await readPsn(api)).status.listening_on).toBe(`239.1.2.3:${nextPort}`);
		await settings.getByRole("button", { name: "Close settings" }).click();
		await expect(page.locator(".psn-setup .psn-status")).toContainText(`239.1.2.3:${nextPort}`);
	});

	test("PSN-COMPATIBILITY-LEGACY-SHOW @ui › a show saved before tracking existed opens with tracking off and nothing bound", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const legacy = await loadCanonicalCopy(api, bench, "psn-legacy", "compact-rig");
		const psn = await readPsn(api, legacy.id);
		expect(psn.revision).toBe(0);
		expect(psn.configuration).toMatchObject({ enabled: false, bindings: [], zones: [] });
		expect(psn.status.error ?? null).toBeNull();
		const tab = await openTrackingTab(page, desk, api.baseUrl);
		await expect(tab.locator(".psn-status")).toHaveText("Not listening. Tracking is switched off.");
		await expect(tab).toContainText("Nothing is bound.");
		await expect(tab.getByRole("alert")).toHaveCount(0);
	});

	test("PSN-COMPATIBILITY-ROUND-TRIP @api › bindings, zones and calibration survive saving and reopening the show", async ({
		api,
		show,
	}) => {
		const rig = await patchTrackingRig(api, show.id);
		const macro = await createMacro(api, show.id, 43, "Zone", "GROUP 3 AT 100");
		const binding = bind(rig, 7);
		await updatePsn(api, {
			port: 56_570,
			stale_after_millis: 1_500,
			calibration: { offset_metres: [1.5, -2, 0.25], rotation_degrees: 90, scale: 1.25 },
			bindings: [binding],
			zones: [
				{
					id: crypto.randomUUID(),
					name: "Downstage",
					min_metres: [-2, -6, -1],
					max_metres: [2, -1, 3],
					tracker_ids: [7],
					enter_macro_id: macro,
					leave_macro_id: null,
					dwell_millis: 400,
				},
			],
		});
		const stored = (await readPsn(api)).configuration;
		expect(stored.bindings).toEqual([binding]);

		const reopened = await reopenFromSavedFile(api, show.id);
		const restored = await readPsn(api, reopened);
		expect(restored.configuration).toEqual(stored);
		expect(restored.points.map((point) => point.fixture_id)).toContain(rig.pointId);

		// And the original show, opened again after another show was active, still holds it.
		await api.openShow(show.id, { transition: "hold_current" });
		expect((await readPsn(api, show.id)).configuration).toEqual(stored);
	});
});
