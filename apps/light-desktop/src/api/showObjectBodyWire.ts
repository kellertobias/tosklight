import type {
	StoredDeskLayout,
	StoredStageLayout,
} from "../features/server/contracts";
import type {
	ShowObjectBodies,
	ShowObjectKind,
} from "../features/showObjects/contracts";
import type { BuiltInWindow, DeskModel, PaneModel } from "../types";
import { decodeRecordedGroupBody } from "./groupRecordingBodyWire";
import {
	arrayAt,
	boundedPositiveIntegerAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { VIRTUAL_PLAYBACKS_PER_PAGE } from "./virtualPlaybackAddress";
import { decodeCueListBody } from "./showObjectCueWire";
import {
	decodePlaybackBody,
	decodePlaybackPageBody,
} from "./showObjectPlaybackWire";
import type { PatchLayer, StoredPreset } from "./types";
import { WireValidationError } from "./wireValidation";

export function decodeShowObjectBody<K extends ShowObjectKind>(
	kind: K,
	value: unknown,
	path: string,
	objectId?: string,
): ShowObjectBodies[K];
export function decodeShowObjectBody(
	kind: ShowObjectKind,
	value: unknown,
	path: string,
	objectId?: string,
): ShowObjectBodies[ShowObjectKind] {
	switch (kind) {
		case "dynamic":
			return decodeDynamic(value, path, objectId);
		case "group":
			return decodeRecordedGroupBody(value, objectId ?? "");
		case "preset":
			return decodePreset(value, path);
		case "cue_list":
			return decodeCueListBody(value, path, objectId);
		case "patch_layer":
			return decodePatchLayer(value, path, objectId);
		case "playback":
			return decodePlaybackBody(value, path, objectId);
		case "playback_page":
			return decodePlaybackPageBody(value, path, objectId);
		case "stage_layout":
			return decodeStageLayout(value, path);
		case "user_layout":
			return decodeUserLayout(value, path);
	}
}

function decodeDynamic(
	value: unknown,
	path: string,
	objectId?: string,
): ShowObjectBodies["dynamic"] {
	try {
		const body = recordAt(value, path);
		const id = stringAt(body.id, `${path}.id`);
		if (objectId && id !== objectId)
			throw new WireValidationError(`${path}.id`, objectId, id);
		arrayAt(body.lanes, `${path}.lanes`);
		arrayAt(body.random_groups, `${path}.random_groups`);
		recordAt(body.target_binding, `${path}.target_binding`);
		recordAt(body.phase, `${path}.phase`);
		recordAt(body.speed, `${path}.speed`);
		return {
			...body,
			phase_mode: body.phase_mode === "per_lane" ? "per_lane" : "uniform",
		} as unknown as ShowObjectBodies["dynamic"];
	} catch (cause) {
		const raw =
			value && typeof value === "object" && !Array.isArray(value)
				? (value as Record<string, unknown>)
				: {};
		const poolNumber =
			typeof raw.pool_number === "number" &&
			Number.isSafeInteger(raw.pool_number) &&
			raw.pool_number >= 1 &&
			raw.pool_number <= 9_999
				? raw.pool_number
				: 9_999;
		const id =
			typeof raw.id === "string" && raw.id ? raw.id : (objectId ?? "invalid");
		const error = cause instanceof Error ? cause.message : String(cause);
		return {
			id,
			pool_number: poolNumber,
			revision: 0,
			name:
				typeof raw.name === "string" && raw.name
					? raw.name
					: `Invalid Dynamic ${poolNumber}`,
			color: typeof raw.color === "string" ? raw.color : "#ef6c73",
			icon: typeof raw.icon === "string" ? raw.icon : "⚠",
			target_binding: { type: "targetless" },
			lanes: [],
			random_groups: [],
			phase_mode: "uniform",
			phase: {
				ordering: { type: "selection" },
				offset_degrees: 0,
				span_degrees: 360,
				block_size: 1,
				repeats: 1,
				wings: false,
				anchors_degrees: [],
			},
			speed: { type: "fixed", duration_millis: 4_000 },
			overall_speed_multiplier: { numerator: 1, denominator: 1 },
			run_mode: "loop",
			default_activation: "start_now",
			activation_boundary: "beat",
			__validationError: `Malformed Dynamic definition: ${error}`,
		} as ShowObjectBodies["dynamic"];
	}
}

function decodePreset(value: unknown, path: string): StoredPreset {
	const body = recordAt(value, path);
	const values = recordAt(body.values, `${path}.values`);
	const decodedValues: StoredPreset["values"] = {};
	for (const [key, rawValue] of Object.entries(values))
		decodedValues[key] = recordAt(rawValue, `${path}.values.${key}`);
	return {
		...body,
		name: plainStringAt(body.name, `${path}.name`),
		number: integerAt(body.number, `${path}.number`),
		values: decodedValues,
	};
}

function plainStringAt(value: unknown, path: string) {
	if (typeof value !== "string")
		throw new WireValidationError(path, "string", value);
	return value;
}

function decodePatchLayer(
	value: unknown,
	path: string,
	objectId?: string,
): PatchLayer {
	const body = recordAt(value, path);
	const id = stringAt(body.id, `${path}.id`);
	if (objectId && id !== objectId)
		throw new WireValidationError(`${path}.id`, objectId, id);
	return {
		...body,
		id,
		name: stringAt(body.name, `${path}.name`),
		order: signedIntegerAt(body.order, `${path}.order`),
	};
}

function decodeStageLayout(value: unknown, path: string): StoredStageLayout {
	const body = recordAt(value, path);
	const positions = position2dMap(body.positions, `${path}.positions`);
	const positions3d =
		body.positions3d == null
			? undefined
			: position3dMap(body.positions3d, `${path}.positions3d`);
	if (body.camera3d != null) {
		const camera = recordAt(body.camera3d, `${path}.camera3d`);
		vector3(camera.position, `${path}.camera3d.position`);
		vector3(camera.target, `${path}.camera3d.target`);
	}
	const version =
		body.version == null
			? undefined
			: integerAt(body.version, `${path}.version`);
	if (version !== undefined && version !== 2)
		throw new WireValidationError(`${path}.version`, "2", version);
	return {
		...body,
		...(version === undefined ? {} : { version }),
		positions,
		...(positions3d === undefined ? {} : { positions3d }),
	};
}

function position2dMap(
	value: unknown,
	path: string,
): StoredStageLayout["positions"] {
	const positions = recordAt(value, path);
	const decoded: StoredStageLayout["positions"] = {};
	for (const [id, raw] of Object.entries(positions)) {
		const position = recordAt(raw, `${path}.${id}`);
		decoded[id] = {
			x: finiteNumberAt(position.x, `${path}.${id}.x`),
			y: finiteNumberAt(position.y, `${path}.${id}.y`),
			rotation: finiteNumberAt(position.rotation, `${path}.${id}.rotation`),
		};
	}
	return decoded;
}

function position3dMap(
	value: unknown,
	path: string,
): NonNullable<StoredStageLayout["positions3d"]> {
	const positions = recordAt(value, path);
	const decoded: NonNullable<StoredStageLayout["positions3d"]> = {};
	for (const [id, raw] of Object.entries(positions)) {
		const position = recordAt(raw, `${path}.${id}`);
		decoded[id] = {
			x: finiteNumberAt(position.x, `${path}.${id}.x`),
			y: finiteNumberAt(position.y, `${path}.${id}.y`),
			z: finiteNumberAt(position.z, `${path}.${id}.z`),
			rotationX: finiteNumberAt(position.rotationX, `${path}.${id}.rotationX`),
			rotationY: finiteNumberAt(position.rotationY, `${path}.${id}.rotationY`),
			rotationZ: finiteNumberAt(position.rotationZ, `${path}.${id}.rotationZ`),
		};
	}
	return decoded;
}

function decodeUserLayout(value: unknown, path: string): StoredDeskLayout {
	const body = recordAt(value, path);
	const desks: DeskModel[] = arrayAt(body.desks, `${path}.desks`).map(
		(rawDesk, deskIndex) => {
			const deskPath = `${path}.desks[${deskIndex}]`;
			const desk = recordAt(rawDesk, deskPath);
			const panes: PaneModel[] = arrayAt(desk.panes, `${deskPath}.panes`).map(
				(rawPane, paneIndex) => {
					const panePath = `${deskPath}.panes[${paneIndex}]`;
					const pane = recordAt(rawPane, panePath);
					const kind = builtInWindowAt(pane.kind, `${panePath}.kind`);
					const virtualFields =
						kind === "virtual_playbacks"
							? decodeVirtualPlaybackPane(pane, panePath)
							: {};
					return {
						...pane,
						...virtualFields,
						id: stringAt(pane.id, `${panePath}.id`),
						kind,
						title: stringAt(pane.title, `${panePath}.title`),
						x: finiteNumberAt(pane.x, `${panePath}.x`),
						y: finiteNumberAt(pane.y, `${panePath}.y`),
						width: finiteNumberAt(pane.width, `${panePath}.width`),
						height: finiteNumberAt(pane.height, `${panePath}.height`),
					};
				},
			);
			return {
				...desk,
				id: stringAt(desk.id, `${deskPath}.id`),
				name: stringAt(desk.name, `${deskPath}.name`),
				panes,
			};
		},
	);
	if (body.windowSettings != null)
		recordAt(body.windowSettings, `${path}.windowSettings`);
	return {
		...body,
		desks,
		activeDeskId: stringAt(body.activeDeskId, `${path}.activeDeskId`),
	};
}

function decodeVirtualPlaybackPane(
	pane: Record<string, unknown>,
	path: string,
): Pick<
	PaneModel,
	| "virtualPlaybackRows"
	| "virtualPlaybackColumns"
	| "virtualPlaybackPageMode"
	| "virtualPlaybackPinnedPage"
> {
	const rows = boundedPositiveIntegerAt(
		pane.virtualPlaybackRows,
		`${path}.virtualPlaybackRows`,
		VIRTUAL_PLAYBACKS_PER_PAGE,
	);
	const columns = boundedPositiveIntegerAt(
		pane.virtualPlaybackColumns,
		`${path}.virtualPlaybackColumns`,
		VIRTUAL_PLAYBACKS_PER_PAGE,
	);
	if (rows * columns > VIRTUAL_PLAYBACKS_PER_PAGE)
		throw new WireValidationError(
			path,
			"a Virtual Playback grid with at most 300 cells",
			pane,
		);
	const mode = pane.virtualPlaybackPageMode;
	if (mode !== "follow_main" && mode !== "pinned")
		throw new WireValidationError(
			`${path}.virtualPlaybackPageMode`,
			"follow_main or pinned",
			mode,
		);
	const pinnedPage = boundedPositiveIntegerAt(
		pane.virtualPlaybackPinnedPage,
		`${path}.virtualPlaybackPinnedPage`,
		127,
	);
	return {
		virtualPlaybackRows: rows,
		virtualPlaybackColumns: columns,
		virtualPlaybackPageMode: mode,
		virtualPlaybackPinnedPage: pinnedPage,
	};
}

const BUILT_IN_WINDOWS: readonly BuiltInWindow[] = [
	"stage",
	"groups",
	"fixtures",
	"presets",
	"cuelists",
	"cuelist_pool",
	"cues",
	"qlists",
	"qlist_pool",
	"qs",
	"playback",
	"playback_pool",
	"cue_list",
	"dynamics",
	"channels",
	"dmx",
	"patch",
	"setup",
	"help",
	"virtual_playbacks",
	"file_manager",
	"text_editor",
];

function builtInWindowAt(value: unknown, path: string): BuiltInWindow {
	const kind = stringAt(value, path);
	if (!BUILT_IN_WINDOWS.includes(kind as BuiltInWindow))
		throw new WireValidationError(path, "known pane kind", value);
	return kind as BuiltInWindow;
}

function vector3(value: unknown, path: string) {
	const vector = arrayAt(value, path);
	if (vector.length !== 3)
		throw new WireValidationError(path, "three-number tuple", value);
	for (const [index, component] of vector.entries())
		finiteNumberAt(component, `${path}[${index}]`);
}

function finiteNumberAt(value: unknown, path: string) {
	if (typeof value !== "number" || !Number.isFinite(value))
		throw new WireValidationError(path, "finite number", value);
	return value;
}

function signedIntegerAt(value: unknown, path: string) {
	if (!Number.isSafeInteger(value))
		throw new WireValidationError(path, "integer", value);
	return value as number;
}
