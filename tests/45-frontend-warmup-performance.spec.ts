import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import type { ApiDriver } from "./bench/core/api";
import { BrowserScenarioWorld } from "./bench/core/browserScenario";
import { expect, test } from "./bench/core/fixtures";
import {
	type FrontendWarmupEvidence,
	measureFrontendWarmup,
} from "./bench/performance/frontendWarmup";
import { activeShowId, seedShowObject } from "./support/catalog";

const profiles = [
	{ name: "demo", cpuThrottle: 1 },
	{ name: "demo", cpuThrottle: 4 },
	{ name: "large", cpuThrottle: 1 },
	{ name: "large", cpuThrottle: 4 },
] as const;

// CPU-throttled performance samples must not compete with another sample from
// this file; that would measure worker contention rather than the desk surface.
test.describe.configure({ mode: "default" });

for (const profile of profiles)
	test(`FRONTEND-WARM-001 @ui @performance › ${profile.name} show at ${profile.cpuThrottle}× CPU switches from retained authority`, async ({
		api,
		bench,
		desk,
		page,
		show,
	}, testInfo) => {
		test.setTimeout(180_000);
		await installProfile(api, profile.name);
		const world = new BrowserScenarioWorld(
			page,
			desk,
			bench,
			api,
			show,
			testInfo,
		);
		const baseline = await measureFrontendWarmup(
			page,
			world,
			desk,
			bench.baseUrl,
			testInfo,
			{ profile: profile.name, cpuThrottle: profile.cpuThrottle, warm: false },
		);
		const warm = await measureFrontendWarmup(
			page,
			world,
			desk,
			bench.baseUrl,
			testInfo,
			{
				profile: profile.name,
				cpuThrottle: profile.cpuThrottle,
				warm: true,
				emitReconciliationEvent: () => emitReconciliationEvent(api),
			},
		);
		assertWarmAcceptance(warm, baseline);
	});

function assertWarmAcceptance(
	warm: FrontendWarmupEvidence,
	baseline: FrontendWarmupEvidence,
) {
	expect(warm.diagnostics.warmup?.status).toBe("ready");
	expect(warm.diagnostics.warmup?.concurrency).toBe(2);
	expect(warm.diagnostics.warmup?.peakActive).toBeLessThanOrEqual(2);
	expect(warm.snapshotRequestsDuringSwitches).toBe(0);
	expect(warm.loadingPlaceholders).toEqual([]);
	expect(baseline.switchSampleCount).toBe(40);
	expect(warm.switchSampleCount).toBe(40);
	expect(warm.switchP95Ms / warm.cpuThrottle).toBeLessThanOrEqual(100);
	expect(warm.diagnostics.firstUsablePaintAt).toBeLessThan(
		performanceMark(warm, "warmup"),
	);
	const warmupStart = Math.min(
		...(warm.diagnostics.warmup?.tasks
			.map(({ startedAt }) => startedAt)
			.filter((value): value is number => value !== null) ?? []),
	);
	const warmupEnd = performanceMark(warm, "warmup");
	expect(
		warm.diagnostics.longTasks.filter(
			({ startedAt, durationMs }) =>
				startedAt <= warmupEnd &&
				startedAt + durationMs >= warmupStart &&
				durationMs / warm.cpuThrottle > 50,
		),
	).toEqual([]);
	expect(warm.diagnostics.warmup?.retainedBytes ?? 0).toBeGreaterThan(0);
	expect(warm.diagnostics.warmup?.retainedBytes ?? 0).toBeLessThanOrEqual(
		warm.diagnostics.warmup?.retainedByteBudget ?? 0,
	);
	expect(baseline.snapshotRequestsDuringSwitches).toBeGreaterThanOrEqual(
		warm.snapshotRequestsDuringSwitches,
	);
}

async function emitReconciliationEvent(api: ApiDriver) {
	const showId = await activeShowId(api);
	await seedShowObject(
		api,
		showId,
		"group",
		`perf-event-${crypto.randomUUID()}`,
		{
			name: "Performance event reconciliation",
			fixtures: [],
			programming: {},
		},
	);
}

function performanceMark(evidence: FrontendWarmupEvidence, kind: "warmup") {
	if (kind === "warmup") {
		const finished = evidence.diagnostics.warmup?.tasks?.map?.(
			(task: { finishedAt: number | null }) => task.finishedAt ?? 0,
		);
		if (finished?.length) return Math.max(...finished);
	}
	return Number.POSITIVE_INFINITY;
}

async function installProfile(api: ApiDriver, profile: "demo" | "large") {
	const demo = await fs.readFile(
		fileURLToPath(new URL("../assets/demo.show", import.meta.url)),
	);
	const created = await api.createShow<{ id: string }>({
		name: `frontend-${profile}-${crypto.randomUUID()}`,
		data_base64: demo.toString("base64"),
		overwrite: false,
	});
	await api.openShow(created.id, { transition: "hold_current" });
	await api.login();
	if (profile === "large") await seedLargeShow(api);
}

async function seedLargeShow(api: ApiDriver) {
	const showId = await activeShowId(api);
	const work: Array<() => Promise<void>> = [];
	const playbackWork: Array<() => Promise<void>> = [];
	const pageWork: Array<() => Promise<void>> = [];
	for (let number = 1; number <= 200; number++) {
		const groupId = `perf-group-${number}`;
		work.push(() =>
			seedShowObject(api, showId, "group", groupId, {
				name: `Performance Group ${number}`,
				fixtures: [],
				programming: {},
			}),
		);
		for (const [family, typeNumber] of [
			["Intensity", 1],
			["Color", 2],
			["Position", 3],
			["Beam", 4],
		] as const) {
			const presetNumber = number + 1_000;
			const id = `${typeNumber}.${presetNumber}`;
			work.push(() =>
				seedShowObject(api, showId, "preset", id, {
					name: `${family} ${presetNumber}`,
					number: presetNumber,
					family,
					values: {},
					group_values: {},
				}),
			);
		}
	}
	for (let number = 1; number <= 100; number++) {
		const playbackNumber = number + 500;
		const cueListId = crypto.randomUUID();
		work.push(() =>
			seedShowObject(api, showId, "cue_list", cueListId, {
				id: cueListId,
				name: `Performance Cuelist ${number}`,
				priority: 0,
				mode: "sequence",
				looped: false,
				chaser_step_millis: 1000,
				speed_group: null,
				intensity_priority_mode: "htp",
				wrap_mode: "off",
				restart_mode: "first_cue",
				force_cue_timing: false,
				disable_cue_timing: false,
				chaser_xfade_percent: 0,
				speed_multiplier: 1,
				cues: [
					{
						id: crypto.randomUUID(),
						number: 1,
						name: "Performance Cue",
						changes: [],
						group_changes: [],
						fade_millis: 0,
						delay_millis: 0,
						trigger: { type: "manual" },
					},
				],
			}),
		);
		playbackWork.push(() =>
			seedShowObject(api, showId, "playback", String(playbackNumber), {
				number: playbackNumber,
				name: `Performance Playback ${playbackNumber}`,
				target: { type: "cue_list", cue_list_id: cueListId },
				buttons: ["go_minus", "go", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: false,
				xfade_millis: 0,
				color: "#20c997",
				flash_release: "release_all",
				protect_from_swap: false,
			}),
		);
	}
	for (let number = 1; number <= 16; number++)
		pageWork.push(() =>
			seedShowObject(api, showId, "playback_page", String(number + 50), {
				number: number + 50,
				name: `Performance Page ${number + 50}`,
				slots: Object.fromEntries(
					Array.from({ length: 64 }, (_, index) => [
						String(index + 1),
						(((number - 1) * 64 + index) % 100) + 501,
					]),
				),
				virtual_playbacks: {},
			}),
		);
	await runInBatches(work);
	await runInBatches(playbackWork);
	await runInBatches(pageWork);
}

async function runInBatches(work: Array<() => Promise<void>>) {
	for (let index = 0; index < work.length; index += 8)
		await Promise.all(
			work.slice(index, index + 8).map((operation) => operation()),
		);
}
