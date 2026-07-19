import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { objects, putObject } from "../catalog";

export async function assignFaderlessMatterPlayback(
	api: Parameters<typeof objects>[0],
): Promise<{
	page: number;
	slot: number;
	emptySlot: number;
	playbackNumber: number;
}> {
	const pages = await objects<any>(api, "playback_page");
	const pagesByNumber = new Map<number, (typeof pages)[number]>(
		pages.map((page) => [Number(page.body.number), page]),
	);
	const emptyPageNumber = Array.from(
		{ length: 127 },
		(_, index) => index + 1,
	).find(
		(page) =>
			Object.keys(pagesByNumber.get(page)?.body.slots ?? {}).length === 0,
	);
	const pageNumber =
		emptyPageNumber ??
		Array.from({ length: 127 }, (_, index) => index + 1).find((page) => {
			const assigned = new Set(
				Object.keys(pagesByNumber.get(page)?.body.slots ?? {}).map(Number),
			);
			return Array.from({ length: 126 }, (_, index) => index + 1).some(
				(slot) => !assigned.has(slot) && !assigned.has(slot + 1),
			);
		});
	expect(pageNumber).toBeDefined();
	const pageState = pagesByNumber.get(pageNumber!);
	const assignedSlots = new Set(
		Object.keys(pageState?.body.slots ?? {}).map(Number),
	);
	const slot = Array.from({ length: 126 }, (_, index) => index + 1).find(
		(candidate) =>
			!assignedSlots.has(candidate) && !assignedSlots.has(candidate + 1),
	);
	expect(slot).toBeDefined();
	const emptySlot = slot! + 1;
	const existingCueList = (await objects<any>(api, "cue_list"))[0];
	const cueListId =
		existingCueList?.id ?? (await createMatterAcceptanceCueList(api));
	const result = await api.request<any>(
		"PUT",
		`/api/v1/playback-pages/${pageNumber}/slots/${slot}`,
		{
			playback: {
				number: 0,
				name: "Matter Button Only",
				target: { type: "cue_list", cue_list_id: cueListId },
				buttons: ["toggle", "none", "none"],
				button_count: 1,
				fader: "master",
				has_fader: false,
				go_activates: true,
				auto_off: false,
				xfade_millis: 0,
				color: "#20c997",
				flash_release: "release_all",
				protect_from_swap: false,
			},
			expected_playback_revision: 0,
			expected_page_revision: pageState?.revision ?? 0,
		},
	);
	return {
		page: pageNumber!,
		slot: slot!,
		emptySlot,
		playbackNumber: result.playback.number,
	};
}

export async function createMatterAcceptanceCueList(
	api: Parameters<typeof objects>[0],
): Promise<string> {
	const fixture = (await objects<any>(api, "patched_fixture"))[0];
	expect(fixture).toBeDefined();
	const id = crypto.randomUUID();
	await putObject(api, "cue_list", id, {
		id,
		name: "Matter Acceptance Cuelist",
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		cues: [
			{
				id: crypto.randomUUID(),
				number: 1,
				name: "Matter On",
				changes: [
					{
						fixture_id: fixture.body.fixture_id,
						attribute: "intensity",
						value: { kind: "normalized", value: 1 },
						automatic_restore: false,
					},
				],
				group_changes: [],
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				phasers: [],
			},
		],
	});
	return id;
}
