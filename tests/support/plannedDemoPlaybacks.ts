import type { ApiDriver } from "../bench/core/api";
import { plannedDemoGroupSpecs } from "./plannedDemoGroups";
import { putPlannedDemoObject } from "./plannedDemoObjects";

interface PatchedTargetFixture {
	fixture_id: string;
	fixture_number: number | null;
	logical_heads?: Array<{ fixture_id: string }>;
}

const CUE_LIST_IDS = {
	start: stableUuid(4, 1),
	acl1: stableUuid(4, 2),
	acl2: stableUuid(4, 3),
	acl3: stableUuid(4, 4),
	acl4: stableUuid(4, 5),
	hazer: stableUuid(4, 6),
	chase: stableUuid(4, 7),
	front: stableUuid(4, 8),
} as const;
export async function installPlannedDemoPlaybacks(
	api: ApiDriver,
	showId: string,
	fixtures: readonly PatchedTargetFixture[],
) {
	const existingPlaybacks = await api.showObjects<any>(showId, "playback");
	const existingPage = await api.showObject<any>(showId, "playback_page", "1");
	const visibleAclOne = existingPlaybacks.find(
		(playback) =>
			playback.body.number === 18 && playback.body.target?.type === "cue_list",
	);
	const visibleFront = existingPlaybacks.find(
		(playback) =>
			playback.body.number === 11 && playback.body.target?.type === "cue_list",
	);
	const cueListIds = {
		...CUE_LIST_IDS,
		acl1: visibleAclOne?.body.target.cue_list_id ?? CUE_LIST_IDS.acl1,
		front: visibleFront?.body.target.cue_list_id ?? CUE_LIST_IDS.front,
	};
	const aclCueListIds = [
		cueListIds.acl1,
		cueListIds.acl2,
		cueListIds.acl3,
		cueListIds.acl4,
	];
	const targetMap = new Map(
		fixtures.flatMap((fixture) =>
			fixture.fixture_number == null
				? []
				: [[fixture.fixture_number, targetIds(fixture)] as const],
		),
	);
	const groups = new Map(
		plannedDemoGroupSpecs().map((group) => [
			group.name,
			group.fixtureNumbers.flatMap((number) => targetMap.get(number) ?? []),
		]),
	);
	const cuelists = [
		cueList(cueListIds.start, "Start", [
			stateCue(1, "Start", [
				...intensity(groups, "Beam Show", 1),
				...intensity(groups, "Beam Auxiliary", 1),
				...intensity(groups, "Wash Show", 1),
				...intensity(groups, "Wash Auxiliary", 1),
				...intensity(groups, "LED Show", 1),
				...intensity(groups, "LED Auxiliary", 1),
				...intensity(groups, "Blinders", 1),
				...attributes(
					groups,
					[
						"Beam Show",
						"Beam Auxiliary",
						"Wash Show",
						"Wash Auxiliary",
						"LED Show",
						"LED Auxiliary",
					],
					{
						"color.red": 1,
						"color.green": 1,
						"color.blue": 1,
					},
				),
				...attributes(
					groups,
					["Beam Show", "Beam Auxiliary", "Wash Show", "Wash Auxiliary"],
					{
						pan: 0.5,
						tilt: 0.5,
					},
				),
			]),
		]),
		...[1, 2, 3, 4].map((number) =>
			cueList(aclCueListIds[number - 1], `ACL ${number}`, [
				stateCue(1, `ACL ${number}`, intensity(groups, `ACL${number}`, 1)),
			]),
		),
		cueList(cueListIds.hazer, "Hazer", [
			stateCue(1, "Haze 20%", intensity(groups, "Hazer", 0.2)),
		]),
		cueList(cueListIds.front, "Front Light", [
			stateCue(1, "Front Light", intensity(groups, "Front Lights", 1)),
		]),
		{
			...cueList(
				cueListIds.chase,
				"ACL Chase",
				[1, 2, 3, 4].map((active) =>
					stateCue(
						active,
						`ACL ${active}`,
						[1, 2, 3, 4].flatMap((number) =>
							intensity(groups, `ACL${number}`, number === active ? 1 : 0),
						),
					),
				),
			),
			mode: "chaser",
			wrap_mode: "reset",
			looped: true,
			speed_group: "D",
			chaser_step_millis: 500,
		},
	];
	for (const cuelist of cuelists)
		await putPlannedDemoObject(api, showId, "cue_list", cuelist.id, cuelist);

	const playbacks = [
		playback(1, "Beam Show Odd", { type: "group", group_id: "6" }),
		playback(2, "Beam Show Even", { type: "group", group_id: "7" }),
		playback(3, "LED Show", { type: "group", group_id: "18" }),
		playback(4, "Wash Show", { type: "group", group_id: "11" }),
		playback(5, "All ACLs", { type: "group", group_id: "32" }),
		playback(6, "Blinders", { type: "group", group_id: "26" }),
		playback(12, "Start", { type: "cue_list", cue_list_id: cueListIds.start }),
		...[
			[1, 18],
			[2, 13],
			[3, 14],
			[4, 15],
		].map(([number, playbackNumber]) =>
			playback(playbackNumber, `ACL ${number}`, {
				type: "cue_list",
				cue_list_id: aclCueListIds[number - 1],
			}),
		),
		playback(16, "Hazer", { type: "cue_list", cue_list_id: cueListIds.hazer }),
		{
			...playback(17, "ACL Chase", {
				type: "cue_list",
				cue_list_id: cueListIds.chase,
			}),
			buttons: ["toggle", "go_minus", "flash"],
		},
		playback(11, "Front Light", {
			type: "cue_list",
			cue_list_id: cueListIds.front,
		}),
	];
	for (const item of playbacks)
		await putPlannedDemoObject(
			api,
			showId,
			"playback",
			String(item.number),
			item,
		);
	await putPlannedDemoObject(api, showId, "playback_page", "1", {
		number: 1,
		name: "Busking",
		slots: Object.fromEntries(
			playbacks.map((item) => [String(item.number), item.number]),
		),
		virtual_playbacks: existingPage?.body.virtual_playbacks ?? {},
	});
	return { cuelists, playbacks };
}

function intensity(groups: Map<string, string[]>, name: string, value: number) {
	const fixtures = groups.get(name);
	if (!fixtures) throw new Error(`Missing generated Group ${name}`);
	return fixtures.map((fixtureId) => [fixtureId, "intensity", value] as const);
}

function attributes(
	groups: Map<string, string[]>,
	names: readonly string[],
	values: Readonly<Record<string, number>>,
) {
	return names.flatMap((name) => {
		const fixtures = groups.get(name);
		if (!fixtures) throw new Error(`Missing generated Group ${name}`);
		return fixtures.flatMap((fixtureId) =>
			Object.entries(values).map(
				([attribute, value]) => [fixtureId, attribute, value] as const,
			),
		);
	});
}

function stateCue(
	number: number,
	name: string,
	changes: ReadonlyArray<readonly [string, string, number]>,
) {
	return {
		id: stableUuid(3, number),
		number,
		name,
		cue_only: false,
		changes: changes.map(([fixture_id, attribute, value]) => ({
			fixture_id,
			attribute,
			value: { kind: "normalized", value },
			automatic_restore: false,
		})),
		group_changes: [],
		fade_millis: 1_000,
		delay_millis: 0,
		trigger: { type: "manual" },
	};
}

function cueList(id: string, name: string, cues: unknown[]) {
	return {
		id,
		name,
		cues,
		mode: "sequence",
		priority: 0,
		looped: false,
		intensity_priority_mode: "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_step_millis: 1_000,
		chaser_xfade_millis: 0,
		speed_group: null,
		speed_multiplier: 1,
	};
}

function playback(
	number: number,
	name: string,
	target: Record<string, unknown>,
) {
	const group = target.type === "group";
	return {
		number,
		name,
		target,
		buttons: group
			? ["select", "flash", "select_dereferenced"]
			: ["go", "go_minus", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

function targetIds(fixture: PatchedTargetFixture) {
	return fixture.logical_heads?.length
		? fixture.logical_heads.map((head) => head.fixture_id)
		: [fixture.fixture_id];
}

function stableUuid(namespace: number, value: number) {
	return `00000000-0000-4${namespace.toString(16).padStart(3, "0")}-8200-${value.toString(16).padStart(12, "0")}`;
}
