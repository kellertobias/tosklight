import type {
	AttributeValue,
	Cue,
	CueList,
	PlaybackDefinition,
	PlaybackPage,
	ProgrammerState,
	VersionedObject,
} from "../../api/types";
import { cueOnlyRestoration } from "./contracts";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

type CueChange = Cue["changes"][number];
type GroupChange = NonNullable<Cue["group_changes"]>[number];

function mergeChanges<
	T extends { fixture_id?: string; group_id?: string; attribute: string },
>(previous: T[], incoming: T[]) {
	return [
		...previous.filter(
			(old) =>
				!incoming.some(
					(next) =>
						next.attribute === old.attribute &&
						next.fixture_id === old.fixture_id &&
						next.group_id === old.group_id,
				),
		),
		...incoming,
	];
}

function fixtureChanges(values: unknown[]): CueChange[] {
	return (
		values as Array<{
			fixture_id: string;
			attribute: string;
			value: AttributeValue;
			fade_millis?: number;
			delay_millis?: number;
		}>
	).map((value) => ({
		fixture_id: value.fixture_id,
		attribute: value.attribute,
		value: value.value,
		...(value.fade_millis == null ? {} : { fade_millis: value.fade_millis }),
		...(value.delay_millis == null ? {} : { delay_millis: value.delay_millis }),
	}));
}

function groupChanges(
	values:
		| ProgrammerState["group_values"]
		| ProgrammerState["preload_group_pending"],
): GroupChange[] {
	return Object.entries(values ?? {}).flatMap(([group_id, attributes]) =>
		Object.entries(attributes).map(([attribute, value]) => ({
			group_id,
			attribute,
			value: value.value as AttributeValue,
			...(value.fade_millis == null ? {} : { fade_millis: value.fade_millis }),
			...(value.delay_millis == null
				? {}
				: { delay_millis: value.delay_millis }),
		})),
	);
}

function initialCueList(id: string, slot: number): CueList {
	return {
		id,
		name: `Cuelist ${slot + 1}`,
		priority: 50,
		mode: "sequence",
		looped: false,
		intensity_priority_mode: "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_step_millis: 1000,
		chaser_xfade_percent: 0,
		speed_group: null,
		speed_multiplier: 1,
		cues: [],
	};
}

function recordedCue(
	current: CueList,
	programmer: ProgrammerState,
	activeCueIndex: number | null,
): Cue {
	const mergeActive = activeCueIndex != null;
	const previousCue = mergeActive ? current.cues[activeCueIndex] : null;
	const cueOnly = localStorage.getItem("light.store-cue-only") === "true";
	const recordingBlind =
		programmer.blind && programmer.preload_capture_programmer !== false;
	const values = recordingBlind
		? programmer.preload_pending
		: programmer.values;
	const groupValues = recordingBlind
		? programmer.preload_group_pending
		: programmer.group_values;
	const cueNumber = previousCue
		? previousCue.number
		: current.cues.length
			? Math.max(...current.cues.map((cue) => cue.number)) + 1
			: 1;
	const restoration = mergeActive
		? { changes: [], group_changes: [] }
		: cueOnlyRestoration(current.cues);
	return {
		number: cueNumber,
		name: previousCue?.name ?? `Cue ${cueNumber}`,
		cue_only: previousCue?.cue_only ?? cueOnly,
		fade_millis: previousCue?.fade_millis ?? 0,
		delay_millis: previousCue?.delay_millis ?? 0,
		trigger: previousCue?.trigger ?? { type: "manual" },
		changes: mergeChanges(
			mergeActive ? (previousCue?.changes ?? []) : restoration.changes,
			fixtureChanges(values ?? []),
		),
		group_changes: mergeChanges(
			mergeActive
				? (previousCue?.group_changes ?? [])
				: restoration.group_changes,
			groupChanges(groupValues),
		),
		phasers: previousCue?.phasers ?? [],
	};
}

async function ensurePlayback(
	model: ServerController,
	showId: string,
	cueList: CueList,
): Promise<VersionedObject<PlaybackDefinition>> {
	const objects = await model.client.objects<PlaybackDefinition>(
		showId,
		"playback",
	);
	const existing = objects.find(
		(item) =>
			item.body.target.type === "cue_list" &&
			item.body.target.cue_list_id === cueList.id,
	);
	if (existing) return existing;
	const used = new Set(objects.map((item) => item.body.number));
	const number = Array.from({ length: 1000 }, (_, index) => index + 1).find(
		(candidate) => !used.has(candidate),
	);
	if (!number) throw new Error("The Cuelist Pool is full");
	const body: PlaybackDefinition = {
		number,
		name: cueList.name,
		target: { type: "cue_list", cue_list_id: cueList.id },
		buttons: ["go", "go_minus", "flash"],
		fader: "master",
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
	};
	await model.client.putObject(showId, "playback", String(number), body, 0);
	return {
		kind: "playback",
		id: String(number),
		body,
		revision: 1,
		updated_at: "",
	};
}

async function assignSlot(
	model: ServerController,
	showId: string,
	pageNumber: number,
	slot: number,
	playbackNumber: number,
) {
	const pages = await model.client.objects<PlaybackPage>(
		showId,
		"playback_page",
	);
	const object = pages.find((item) => item.body.number === pageNumber);
	const page = object?.body ?? {
		number: pageNumber,
		name: pageNumber === 1 ? "Main" : `Page ${pageNumber}`,
		slots: {},
	};
	await model.client.putObject(
		showId,
		"playback_page",
		String(pageNumber),
		{ ...page, slots: { ...page.slots, [slot + 1]: playbackNumber } },
		object?.revision ?? 0,
	);
}

async function storePlayback(
	model: ServerController,
	slot: number,
	cueListId?: string,
	explicitPageNumber?: number,
) {
	const { bootstrap, client, playbacks, refresh, session, setError } = model;
	try {
		if (!bootstrap?.active_show || !session)
			throw new Error("Open a show before storing a Cue");
		const programmers = await client.programmers();
		const programmer = programmers.find(
			(item) => item.session_id === session.session_id,
		);
		if (!programmer) throw new Error("The current programmer is unavailable");
		const objects = await client.objects<CueList>(
			bootstrap.active_show.id,
			"cue_list",
		);
		const existing = cueListId
			? objects.find((item) => item.id === cueListId)
			: undefined;
		const id = existing?.id ?? crypto.randomUUID();
		const current = existing?.body ?? initialCueList(id, slot);
		const active = playbacks?.active.find((item) => item.cue_list_id === id);
		const activeCueIndex =
			localStorage.getItem("light.store-merge-active-cue") === "true" &&
			active &&
			current.cues[active.cue_index]
				? active.cue_index
				: null;
		const cue = recordedCue(current, programmer, activeCueIndex);
		const cues =
			activeCueIndex == null
				? [...current.cues, cue]
				: current.cues.map((item, index) =>
						index === activeCueIndex ? cue : item,
					);
		const saved = { ...current, cues };
		await client.putObject(
			bootstrap.active_show.id,
			"cue_list",
			id,
			saved,
			existing?.revision ?? 0,
		);
		const playback = await ensurePlayback(
			model,
			bootstrap.active_show.id,
			saved,
		);
		await assignSlot(
			model,
			bootstrap.active_show.id,
			explicitPageNumber ?? playbacks?.active_page ?? 1,
			slot,
			playback.body.number,
		);
		await refresh();
		const recordingBlind =
			programmer.blind && programmer.preload_capture_programmer !== false;
		if (!recordingBlind) {
			await client.poolPlaybackAction(playback.body.number, "go-to", {
				cue_number: cue.number,
			});
			await refresh();
		}
		setError(null);
	} catch (reason) {
		setError(reason instanceof Error ? reason.message : String(reason));
	}
}

export function createStorePlaybackValue(
	model: ServerController,
): Pick<ServerContextValue, "storePlayback"> {
	return {
		storePlayback: (slot, cueListId, pageNumber) =>
			storePlayback(model, slot, cueListId, pageNumber),
	};
}
