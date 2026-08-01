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
} as const;
const ACL_CUE_LIST_IDS = [
	CUE_LIST_IDS.acl1,
	CUE_LIST_IDS.acl2,
	CUE_LIST_IDS.acl3,
	CUE_LIST_IDS.acl4,
] as const;

export async function installPlannedDemoPlaybacks(
	api: ApiDriver,
	showId: string,
	fixtures: readonly PatchedTargetFixture[],
) {
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
		cueList(CUE_LIST_IDS.start, "Start", [
			stateCue(1, "Start", [
				...intensity(groups, "Profile All", 1),
				...intensity(groups, "Wash All", 1),
				...intensity(groups, "LED All", 1),
				...intensity(groups, "Blinders", 1),
				...attributes(groups, ["Profile All", "Wash All", "LED All"], {
					"color.red": 1,
					"color.green": 1,
					"color.blue": 1,
				}),
				...attributes(groups, ["Profile All", "Wash All"], {
					pan: 0.5,
					tilt: 0.5,
				}),
			]),
		]),
		...[1, 2, 3, 4].map((number) =>
			cueList(ACL_CUE_LIST_IDS[number - 1], `ACL ${number}`, [
				stateCue(1, `ACL ${number}`, intensity(groups, `ACL ${number}`, 1)),
			]),
		),
		cueList(CUE_LIST_IDS.hazer, "Hazer", [
			stateCue(1, "Haze 20%", intensity(groups, "Hazers", 0.2)),
		]),
		{
			...cueList(
				CUE_LIST_IDS.chase,
				"ACL Chase",
				[1, 2, 3, 4].map((active) =>
					stateCue(
						active,
						`ACL ${active}`,
						[1, 2, 3, 4].flatMap((number) =>
							intensity(groups, `ACL ${number}`, number === active ? 1 : 0),
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
		playback(1, "Show Profile Odd", { type: "group", group_id: "21" }),
		playback(2, "Show Profile Even", { type: "group", group_id: "22" }),
		playback(3, "Show LED", { type: "group", group_id: "17" }),
		playback(4, "Show Wash", { type: "group", group_id: "15" }),
		playback(5, "All ACLs", { type: "group", group_id: "35" }),
		playback(6, "Blinders", { type: "group", group_id: "36" }),
		playback(11, "Start", {
			type: "cue_list",
			cue_list_id: CUE_LIST_IDS.start,
		}),
		...[1, 2, 3, 4].map((number) =>
			playback(11 + number, `ACL ${number}`, {
				type: "cue_list",
				cue_list_id: ACL_CUE_LIST_IDS[number - 1],
			}),
		),
		playback(16, "Hazer", {
			type: "cue_list",
			cue_list_id: CUE_LIST_IDS.hazer,
		}),
		playback(17, "ACL Chase", {
			type: "cue_list",
			cue_list_id: CUE_LIST_IDS.chase,
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
		virtual_playbacks: {},
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
