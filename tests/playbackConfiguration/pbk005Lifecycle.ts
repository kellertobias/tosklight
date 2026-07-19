import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import {
	type BenchUiContext,
	expect,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	activePlayback,
	createCueList,
	definition,
	installPlaybacks,
	logicalUniverse,
	playbackAt,
	playbackSnapshot,
	poolAction,
	prepareShow,
	updatePlayback,
} from "./helpers";
import type { PreparedShow } from "./models";

interface LifecycleArrangement {
	prepared: PreparedShow;
	aBefore: any;
}

async function prepareLifecyclePlaybacks({
	api,
	bench,
}: Pick<BenchUiContext, "api" | "bench">): Promise<LifecycleArrangement> {
	const prepared = await prepareShow(
		api,
		bench,
		"pbk-005-stack",
		"default-stage",
	);
	const aId = await createCueList(
		api,
		prepared.fixtures,
		"A",
		[0.2],
		0,
		0,
		[1, 2],
		false,
	);
	const bId = await createCueList(
		api,
		prepared.fixtures,
		"B",
		[0.8],
		0,
		0,
		[1, 2, 3],
		false,
	);
	const cId = await createCueList(
		api,
		prepared.fixtures,
		"C",
		[0.6],
		0,
		0,
		[4],
		false,
	);
	const dId = await createCueList(
		api,
		prepared.fixtures,
		"D protected",
		[0.4],
		0,
		0,
		[5],
		false,
	);
	const a = definition(
		51,
		"A",
		{ type: "cue_list", cue_list_id: aId },
		{ buttons: ["go", "flash", "none"] },
	);
	const b = definition(
		52,
		"B",
		{ type: "cue_list", cue_list_id: bId },
		{ buttons: ["flash", "temp", "swap"] },
	);
	const c = definition(
		53,
		"C",
		{ type: "cue_list", cue_list_id: cId },
		{ auto_off: false },
	);
	const d = definition(
		54,
		"D protected",
		{ type: "cue_list", cue_list_id: dId },
		{ auto_off: false, protect_from_swap: true },
	);
	await installPlaybacks(api, [a, b, c, d], {
		1: 51,
		2: 52,
		3: 53,
		4: 54,
	});
	await poolAction(api, 51, "on");
	return { prepared, aBefore: await activePlayback(api, 51) };
}

async function verifyFlashAndTempModes(
	api: ApiDriver,
	aBefore: any,
): Promise<void> {
	await poolAction(api, 52, "flash", { pressed: true });
	expect((await activePlayback(api, 51)).enabled).toBe(true);
	expect(await activePlayback(api, 52)).toMatchObject({
		flash: true,
		temporary_active: true,
	});
	await poolAction(api, 52, "flash", { pressed: false });
	expect(await activePlayback(api, 51)).toMatchObject({
		enabled: true,
		cue_index: aBefore.cue_index,
		activated_at: aBefore.activated_at,
	});
	expect(
		(await playbackSnapshot(api)).active.some(
			(item: any) => item.playback_number === 52 && item.temporary_active,
		),
	).toBe(false);

	await poolAction(api, 52, "temp");
	expect(await activePlayback(api, 52)).toMatchObject({
		temporary_active: true,
	});
	expect((await activePlayback(api, 51)).enabled).toBe(true);
	await poolAction(api, 52, "temp");
	expect(
		(await playbackSnapshot(api)).active.some(
			(item: any) => item.playback_number === 52 && item.temporary_active,
		),
	).toBe(false);
	expect(await activePlayback(api, 51)).toMatchObject({
		enabled: true,
		activated_at: aBefore.activated_at,
	});

	await updatePlayback(api, 2, (current) => ({
		...current,
		flash_release: "release_intensity_only",
	}));
	await poolAction(api, 52, "flash", { pressed: true });
	await poolAction(api, 52, "flash", { pressed: false });
	expect(await activePlayback(api, 52)).toMatchObject({
		enabled: true,
		master: 0,
		temporary_active: false,
	});
	await updatePlayback(api, 2, (current) => ({
		...current,
		flash_release: "release_all",
	}));
	await poolAction(api, 52, "off");
	await poolAction(api, 52, "flash", { pressed: true });
	await poolAction(api, 52, "flash", { pressed: false });
	expect(
		(await playbackSnapshot(api)).active.some(
			(item: any) => item.playback_number === 52 && item.enabled,
		),
	).toBe(false);
}

async function verifyAutoOff(api: ApiDriver): Promise<void> {
	await poolAction(api, 51, "on");
	await updatePlayback(api, 1, (current) => ({ ...current, auto_off: true }));
	await poolAction(api, 52, "on");
	expect((await activePlayback(api, 51)).enabled).toBe(false);
	await updatePlayback(api, 1, (current) => ({
		...current,
		auto_off: false,
	}));
	await poolAction(api, 51, "on");
	await poolAction(api, 52, "off");
	await poolAction(api, 52, "on");
	expect((await activePlayback(api, 51)).enabled).toBe(true);
}

async function verifySwapProtection(
	api: ApiDriver,
	bench: BenchUiContext["bench"],
	prepared: PreparedShow,
): Promise<void> {
	await poolAction(api, 53, "on");
	expect(await activePlayback(api, 53)).toMatchObject({
		enabled: true,
		master: 1,
	});
	await poolAction(api, 54, "on");
	expect(await activePlayback(api, 53)).toMatchObject({
		enabled: true,
		master: 1,
	});
	expect(await activePlayback(api, 54)).toMatchObject({
		enabled: true,
		master: 1,
	});
	const aSwapBefore = await activePlayback(api, 51);
	const cSwapBefore = await activePlayback(api, 53);
	const dSwapBefore = await activePlayback(api, 54);
	const beforeSwap = logicalUniverse(await bench.tick(3_000));
	const beforeResolved = await api.request<any>("GET", "/api/v1/visualization");
	const resolvedLevel = (fixtureNumber: number) =>
		beforeResolved.values.find(
			(item: any) =>
				item.fixture_id === prepared.fixtures[fixtureNumber] &&
				item.attribute === "intensity",
		)?.value?.value;
	expect(resolvedLevel(4)).toBeCloseTo(0.6, 5);
	expect(resolvedLevel(5)).toBeCloseTo(0.4, 5);
	expect(beforeSwap[3]).toBeGreaterThan(0);
	expect(beforeSwap[4]).toBeGreaterThan(0);
	await poolAction(api, 52, "swap", { pressed: true });
	expect(await activePlayback(api, 52)).toMatchObject({
		swap_active: true,
		temporary_active: true,
	});
	expect((await activePlayback(api, 51)).enabled).toBe(true);
	expect((await activePlayback(api, 53)).enabled).toBe(true);
	expect((await activePlayback(api, 54)).enabled).toBe(true);
	const duringSwap = logicalUniverse(await bench.tick(0));
	expect(duringSwap[3]).toBe(0);
	expect(duringSwap[4]).toBeGreaterThan(0);
	await poolAction(api, 52, "swap", { pressed: false });
	expect(await activePlayback(api, 51)).toMatchObject({
		cue_index: aSwapBefore.cue_index,
		fader_position: aSwapBefore.fader_position,
		activated_at: aSwapBefore.activated_at,
	});
	expect(await activePlayback(api, 53)).toMatchObject({
		enabled: true,
		cue_index: cSwapBefore.cue_index,
		fader_position: cSwapBefore.fader_position,
		activated_at: cSwapBefore.activated_at,
	});
	expect(await activePlayback(api, 54)).toMatchObject({
		enabled: true,
		cue_index: dSwapBefore.cue_index,
		fader_position: dSwapBefore.fader_position,
		activated_at: dSwapBefore.activated_at,
	});
	const afterSwap = logicalUniverse(await bench.tick(0));
	expect(afterSwap[3]).toBeGreaterThan(0);
	expect(afterSwap[4]).toBeGreaterThan(0);
	await api.request("POST", `/api/v1/shows/${prepared.showId}/open`, {
		transition: "hold_current",
	});
	expect((await playbackAt(api, 1, 4)).body.protect_from_swap).toBe(true);
	expect((await playbackAt(api, 1, 1)).body.auto_off).toBe(false);
}

export async function runPbk005LifecycleScenario({
	api,
	bench,
}: BenchUiContext): Promise<void> {
	const { prepared, aBefore } = await prepareLifecyclePlaybacks({ api, bench });
	await verifyFlashAndTempModes(api, aBefore);
	await verifyAutoOff(api);
	await verifySwapProtection(api, bench, prepared);
}
