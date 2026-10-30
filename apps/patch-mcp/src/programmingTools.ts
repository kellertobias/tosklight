/**
 * Programming the desk: selection, values, Presets, Cues, playbacks and the command line.
 *
 * Every action goes through the same desk routes an operator's keypad, OSC surface or the Control
 * app uses, under this server's desk alias. With `TOSKLIGHT_DESK_ID` set to the operator's desk,
 * the command line these tools type on is the one shown on that desk; with any other alias they
 * program a separate desk and leave the operator's line alone.
 */

import { fixtureLabel, type PatchBackend, type PatchedFixture, UnsupportedByProduct } from "./backend";
import { commandRisk, type StoredObjects } from "./commandSafety";
import { Desk } from "./desk";
import { snapshotImage } from "./programmingSnapshot";
import { mcpContent, type Tool } from "./toolSchema";

interface ObjectRecord<T> {
	id: string;
	revision: number;
	body: T;
}

interface CueBody {
	number: string;
	name: string;
	fade_millis: number;
	delay_millis: number;
	cue_only?: boolean;
	changes: Array<{ fixture_id: string; attribute: string; value: unknown; preset_reference?: { preset_instance_id: string } | null; automatic_restore?: boolean }>;
	group_changes?: Array<{ group_id: string; attribute: string; value: unknown; preset_reference?: { preset_instance_id: string } | null }>;
}

interface CueListBody {
	id: string;
	name: string;
	pool_number?: number | null;
	mode: string;
	cues: CueBody[];
}

interface CommandOutcome {
	outcome: "accepted" | "choice_required" | "rejected";
	action?: string;
	applied?: number | null;
	warning?: string | null;
	error?: string;
	pending_choice?: unknown;
	command_line: { text: string; revision: number };
}

/** The tools below need the Control desk; the Architect has no Programmer. */
function controlDesk(desk: PatchBackend): Desk {
	if (!(desk instanceof Desk)) throw new UnsupportedByProduct(desk.product, "program cues, values or playbacks");
	return desk;
}

async function activeShowId(desk: Desk): Promise<string> {
	const bootstrap = await desk.request<{ active_show: { id: string } | null }>("GET", "/api/v2/bootstrap", undefined, false);
	if (!bootstrap.active_show) throw new Error("The desk has no open show.");
	return bootstrap.active_show.id;
}

async function objects<T>(desk: Desk, kind: string): Promise<Array<ObjectRecord<T>>> {
	const showId = await activeShowId(desk);
	const snapshot = await desk.request<{ objects: Array<ObjectRecord<T>> }>("GET", `/api/v2/objects/${kind}`, undefined, true, undefined, { showId });
	return snapshot.objects;
}

async function fixtureNames(desk: Desk) {
	const patch = await desk.patch();
	return new Map(patch.fixtures.map((fixture: PatchedFixture) => [fixture.fixture_id, fixtureLabel(fixture) ?? fixture.name]));
}

async function storedObjects(desk: Desk): Promise<StoredObjects> {
	const [cueLists, groups, presets] = await Promise.all([
		objects<CueListBody>(desk, "cue_list"),
		objects<{ id: string }>(desk, "group"),
		objects<{ family?: string; number: number }>(desk, "preset"),
	]);
	return {
		cues: new Map(cueLists.map((list) => [Number(list.body.pool_number), new Set(list.body.cues.map((cue) => cue.number))])),
		groups: new Set(groups.map((group) => String(group.body.id ?? group.id))),
		presets: new Set(presets.map((preset) => `${String(preset.body.family ?? "").toLowerCase()}:${preset.body.number}`)),
	};
}

/** Type a line on the shared command line and press ENT. */
async function execute(desk: Desk, command: string): Promise<CommandOutcome> {
	const showId = await activeShowId(desk);
	const result = await desk.request<CommandOutcome>(
		"POST",
		"/api/v2/command-line/execute",
		{ command, request_id: crypto.randomUUID() },
		true,
		undefined,
		{ showId },
	);
	if (result.outcome === "rejected") throw new Error(`The desk rejected "${command}": ${result.error}`);
	return result;
}

async function pressKey(desk: Desk, key: string, shifted = false): Promise<CommandOutcome> {
	const showId = await activeShowId(desk);
	return desk.request<CommandOutcome>(
		"POST",
		"/api/v2/command-line/keys",
		{ key, phase: "press", request_id: crypto.randomUUID(), ...(shifted ? { gesture: { kind: "regular", shifted: true } } : {}) },
		true,
		undefined,
		{ showId },
	);
}

/** Refuse a line that would overwrite or remove stored programming unless the caller confirmed. */
async function guarded(desk: Desk, command: string, confirm: unknown): Promise<CommandOutcome> {
	if (confirm !== true) {
		const risk = commandRisk(command, await storedObjects(desk));
		if (risk.destructive)
			throw new Error(`"${command}" was not run because ${risk.reason}. Call again with confirm: true to overwrite or remove it.`);
	}
	return execute(desk, command);
}

const commandResult = (result: CommandOutcome) => ({
	outcome: result.outcome,
	action: result.action ?? null,
	applied: result.applied ?? null,
	warning: result.warning ?? null,
	pending_choice: result.pending_choice ?? null,
	command_line: result.command_line.text,
});

const seconds = (millis: number) => Math.round(millis) / 1000;
const number = (value: unknown) => {
	const parsed = Number(value);
	if (!Number.isFinite(parsed)) throw new Error(`${String(value)} is not a number`);
	return Math.round(parsed * 1000) / 1000;
};
const fade = (input: Record<string, any>) => (input.fade_seconds == null ? "" : ` TIME ${number(input.fade_seconds)}`);
const confirmProperty = {
	confirm: {
		type: "boolean",
		description: "Required as true when the command would overwrite, update, move or delete stored programming.",
	},
};

const readTools: Tool[] = [
	{
		name: "get_show_summary",
		description: "The open show, how much it holds, the shared command line, and whether Preload (blind programming) is pending.",
		inputSchema: { type: "object", properties: {} },
		async run(backend) {
			const desk = controlDesk(backend);
			const bootstrap = await desk.request<{ active_show: { id: string; name: string } | null }>("GET", "/api/v2/bootstrap", undefined, false);
			const [patch, groups, presets, cueLists, pages, programmers, line] = await Promise.all([
				desk.patch(),
				objects(desk, "group"),
				objects(desk, "preset"),
				objects(desk, "cue_list"),
				objects(desk, "playback_page"),
				desk.request<Array<{ preload_pending: unknown[]; preload_group_pending: Record<string, unknown> }>>("GET", "/api/v2/programmers"),
				desk.request<{ text: string }>("GET", "/api/v2/command-line"),
			]);
			return {
				show: bootstrap.active_show ? { id: bootstrap.active_show.id, name: bootstrap.active_show.name } : null,
				fixtures: patch.fixtures.length,
				groups: groups.length,
				presets: presets.length,
				cue_lists: cueLists.length,
				playback_pages: pages.length,
				command_line: line.text,
				preload_pending: programmers.some((programmer) => programmer.preload_pending.length > 0 || Object.keys(programmer.preload_group_pending).length > 0),
			};
		},
	},
	{
		name: "list_groups",
		description: "Every Group with its number, name and ordered fixture numbers. Order matters: spreads and THRU values follow it.",
		inputSchema: { type: "object", properties: {} },
		async run(backend) {
			const desk = controlDesk(backend);
			const [groups, names] = await Promise.all([objects<{ id: string; name: string; fixtures?: string[] }>(desk, "group"), fixtureNames(desk)]);
			return groups.map((group) => ({
				group: group.body.id ?? group.id,
				name: group.body.name,
				fixtures: (group.body.fixtures ?? []).map((id) => names.get(id) ?? id),
			}));
		},
	},
	{
		name: "list_presets",
		description: "Stored Presets with family, number and name. Recall one with apply_preset.",
		inputSchema: {
			type: "object",
			properties: { family: { type: "string", enum: ["intensity", "color", "position", "beam", "focus", "mixed"] } },
		},
		async run(backend, input) {
			const presets = await objects<{ family?: string; number: number; name: string }>(controlDesk(backend), "preset");
			return presets
				.map((preset) => ({ family: String(preset.body.family ?? "mixed").toLowerCase(), number: preset.body.number, name: preset.body.name }))
				.filter((preset) => !input.family || preset.family === input.family)
				.sort((left, right) => left.family.localeCompare(right.family) || left.number - right.number);
		},
	},
	{
		name: "list_cue_lists",
		description: "Cuelists with their Cues, timing and how many values each Cue stores. Pass cue_list and include_values for the stored values of one Cuelist.",
		inputSchema: {
			type: "object",
			properties: {
				cue_list: { type: "number", description: "Cuelist pool number; omit for all Cuelists." },
				include_values: { type: "boolean", description: "Include every stored fixture and Group value per Cue." },
			},
		},
		async run(backend, input) {
			const desk = controlDesk(backend);
			const [lists, names, presets] = await Promise.all([
				objects<CueListBody>(desk, "cue_list"),
				fixtureNames(desk),
				objects<{ instance_id?: string; name: string; number: number; family?: string }>(desk, "preset"),
			]);
			const presetNames = new Map(presets.flatMap((preset) => (preset.body.instance_id ? [[preset.body.instance_id, `${preset.body.family ?? "Preset"} ${preset.body.number} ${preset.body.name}`.trim()]] : [])));
			return lists
				.filter((list) => input.cue_list == null || list.body.pool_number === Number(input.cue_list))
				.map((list) => ({
					cue_list: list.body.pool_number ?? null,
					name: list.body.name,
					mode: list.body.mode,
					cues: list.body.cues.map((cue) => ({
						cue: cue.number,
						name: cue.name,
						fade_seconds: seconds(cue.fade_millis),
						delay_seconds: seconds(cue.delay_millis),
						cue_only: Boolean(cue.cue_only),
						stored_values: cue.changes.filter((change) => !change.automatic_restore).length + (cue.group_changes?.length ?? 0),
						...(input.include_values
							? {
									values: [
										...cue.changes.filter((change) => !change.automatic_restore).map((change) => ({
											fixture: names.get(change.fixture_id) ?? change.fixture_id,
											attribute: change.attribute,
											value: change.value,
											preset: change.preset_reference ? presetNames.get(change.preset_reference.preset_instance_id) ?? "missing preset" : null,
										})),
										...(cue.group_changes ?? []).map((change) => ({
											group: change.group_id,
											attribute: change.attribute,
											value: change.value,
											preset: change.preset_reference ? presetNames.get(change.preset_reference.preset_instance_id) ?? "missing preset" : null,
										})),
									],
								}
							: {}),
					})),
				}));
		},
	},
	{
		name: "list_playbacks",
		description: "Playback pages and what each playback position runs.",
		inputSchema: { type: "object", properties: {} },
		async run(backend) {
			const pages = await objects<{ number: number; name: string; assignments: Record<string, unknown> }>(controlDesk(backend), "playback_page");
			return pages.map((page) => ({ page: page.body.number, name: page.body.name, playbacks: page.body.assignments }));
		},
	},
	{
		name: "get_programmer",
		description: "The current selection (in order) and the Programmer's fixture and Group values, live and pending in Preload.",
		inputSchema: { type: "object", properties: {} },
		async run(backend) {
			const desk = controlDesk(backend);
			const [programmers, names] = await Promise.all([
				desk.request<Array<Record<string, any>>>("GET", "/api/v2/programmers"),
				fixtureNames(desk),
			]);
			const programmer = programmers[0];
			if (!programmer) return { selected: [], values: [], group_values: {} };
			return {
				selected: (programmer.selected as string[]).map((id) => names.get(id) ?? id),
				values: (programmer.values as Array<{ fixture_id: string; attribute: string; value: unknown }>).map((value) => ({ fixture: names.get(value.fixture_id) ?? value.fixture_id, attribute: value.attribute, value: value.value })),
				group_values: programmer.group_values,
				preload_pending: (programmer.preload_pending as Array<{ fixture_id: string; attribute: string; value: unknown }>).map((value) => ({ fixture: names.get(value.fixture_id) ?? value.fixture_id, attribute: value.attribute, value: value.value })),
				preload_group_pending: programmer.preload_group_pending,
			};
		},
	},
	{
		name: "get_command_line",
		description: "The desk's shared command line as every surface shows it.",
		inputSchema: { type: "object", properties: {} },
		run: (backend) => controlDesk(backend).request("GET", "/api/v2/command-line"),
	},
];

const actTools: Tool[] = [
	{
		name: "run_command",
		description:
			"Type a line on the desk's one shared command line and press ENT, with the keypad's own grammar (e.g. `GROUP 3 AT 50`, `FIXTURE 1 THRU 4 AT POSITION 30 DIV 10`, `RECORD CUELIST 4 CUE 5 TIME 3`). Lines that delete, move, copy, update or record over existing programming need confirm: true.",
		inputSchema: {
			type: "object",
			properties: { command: { type: "string" }, ...confirmProperty },
			required: ["command"],
		},
		async run(backend, input) {
			return commandResult(await guarded(controlDesk(backend), String(input.command), input.confirm));
		},
	},
	{
		name: "select",
		description: "Select fixtures or a Group on the shared command line. `fixtures` uses keypad syntax such as `1 THRU 6 + 9`.",
		inputSchema: {
			type: "object",
			properties: {
				fixtures: { type: "string", description: "Fixture numbers and ranges, e.g. `1 THRU 6 + 9`." },
				group: { type: ["number", "string"], description: "A Group number." },
			},
		},
		async run(backend, input) {
			const desk = controlDesk(backend);
			if (input.group != null) return commandResult(await execute(desk, `GROUP ${String(input.group)}`));
			if (input.fixtures) return commandResult(await execute(desk, `FIXTURE ${String(input.fixtures)}`));
			throw new Error("Pass fixtures or group.");
		},
	},
	{
		name: "set_values",
		description:
			"Set values on the current selection. Intensity in percent; color as Color Intent red/green/blue/white in percent; position as Pan/Tilt in degrees; zoom in beam degrees; focus in percent. An optional fade applies to each value.",
		inputSchema: {
			type: "object",
			properties: {
				intensity: { type: "number" },
				color: {
					type: "object",
					properties: { red: { type: "number" }, green: { type: "number" }, blue: { type: "number" }, white: { type: "number" } },
				},
				position: { type: "object", properties: { pan: { type: "number" }, tilt: { type: "number" } } },
				zoom_degrees: { type: "number" },
				focus: { type: "number" },
				fade_seconds: { type: "number" },
			},
		},
		async run(backend, input) {
			const desk = controlDesk(backend);
			const part = (value: unknown) => (value == null ? "" : String(number(value)));
			const lines: string[] = [];
			if (input.intensity != null) lines.push(`AT ${number(input.intensity)}`);
			if (input.color) {
				const { red, green, blue, white } = input.color;
				lines.push(`AT COLOR ${[part(red), part(green), part(blue), ...(white == null ? [] : [part(white)])].join(" DIV ")}`.replace(/ +/g, " ").trim());
			}
			if (input.position) lines.push(`AT POSITION ${part(input.position.pan)} DIV ${part(input.position.tilt)}`.replace(/ +/g, " ").trim());
			if (input.zoom_degrees != null || input.focus != null) lines.push(`AT FOCUS ${part(input.focus)} DIV ${part(input.zoom_degrees)}`.replace(/ +/g, " ").trim());
			if (!lines.length) throw new Error("Pass at least one value.");
			const results = [];
			for (const line of lines) results.push(commandResult(await execute(desk, `${line}${fade(input)}`)));
			return results;
		},
	},
	{
		name: "apply_preset",
		description: "Recall a Preset onto the current selection, e.g. family color, number 3.",
		inputSchema: {
			type: "object",
			properties: {
				family: { type: "string", enum: ["intensity", "color", "position", "beam", "focus"] },
				number: { type: "number" },
				fade_seconds: { type: "number" },
			},
			required: ["family", "number"],
		},
		async run(backend, input) {
			return commandResult(await execute(controlDesk(backend), `AT ${String(input.family).toUpperCase()} PRESET ${number(input.number)}${fade(input)}`));
		},
	},
	{
		name: "clear",
		description: "Clear the selection, or with values: true also the Programmer's values (CLR pressed twice).",
		inputSchema: { type: "object", properties: { values: { type: "boolean" } } },
		async run(backend, input) {
			const desk = controlDesk(backend);
			const first = await pressKey(desk, "CLR");
			if (!input.values) return commandResult(first);
			return commandResult(await pressKey(desk, "CLR"));
		},
	},
	{
		name: "record_cue",
		description:
			"Record the Programmer as a Cue. Without `cue` it appends to the Cuelist. Recording onto an existing Cue number needs confirm: true. `merge` adds to an existing Cue instead of replacing it (still needs confirm).",
		inputSchema: {
			type: "object",
			properties: {
				cue_list: { type: "number" },
				cue: { type: "string", description: "Cue number such as `5` or `2.1`." },
				fade_seconds: { type: "number" },
				merge: { type: "boolean" },
				...confirmProperty,
			},
			required: ["cue_list"],
		},
		async run(backend, input) {
			const command = `RECORD${input.merge ? " +" : ""} CUELIST ${number(input.cue_list)}${input.cue == null ? "" : ` CUE ${String(input.cue)}`}${fade(input)}`;
			return commandResult(await guarded(controlDesk(backend), command, input.confirm));
		},
	},
	{
		name: "update_cue",
		description:
			"Update an existing Cue from the Programmer. mode: update (values the Cue already stores), tracked (the Cue each value comes from), known (attributes the Cuelist already knows), all (everything). Always needs confirm: true.",
		inputSchema: {
			type: "object",
			properties: {
				cue_list: { type: "number" },
				cue: { type: "string" },
				mode: { type: "string", enum: ["update", "tracked", "known", "all"] },
				...confirmProperty,
			},
			required: ["cue_list", "cue"],
		},
		async run(backend, input) {
			const mode = { update: "", tracked: " -", known: " + +", all: " +" }[String(input.mode ?? "update") as "update"] ?? "";
			return commandResult(await guarded(controlDesk(backend), `UPDATE${mode} CUELIST ${number(input.cue_list)} CUE ${String(input.cue)}`, input.confirm));
		},
	},
	{
		name: "playback",
		description: "Operate a playback: go, back, release, or set its master fader (0–100%). Without page, the desk's current page is used.",
		inputSchema: {
			type: "object",
			properties: {
				position: { type: "number" },
				page: { type: "number" },
				action: { type: "string", enum: ["go", "back", "release", "master"] },
				value: { type: "number", description: "Master level in percent for action master." },
			},
			required: ["position", "action"],
		},
		async run(backend, input) {
			const desk = controlDesk(backend);
			const showId = await activeShowId(desk);
			const position = number(input.position);
			const address = input.page == null ? { kind: "current_page", position } : { kind: "playback", page: number(input.page), position };
			const send = (action: Record<string, unknown>) =>
				desk.request("POST", "/api/v2/playback-actions", { address, action, control_source: { surface_id: "mcp", control_id: position } }, true, undefined, { showId });
			switch (input.action) {
				case "go":
				case "back":
					await send({ type: input.action, pressed: true });
					return send({ type: input.action, pressed: false });
				case "release":
					return send({ type: "release" });
				case "master":
					return send({ type: "master_absolute", value: Math.min(1, Math.max(0, number(input.value ?? 100) / 100)) });
				default:
					throw new Error("action must be go, back, release or master");
			}
		},
	},
	{
		name: "preload",
		description:
			"Blind programming. enter: the following values go into Preload and do not change live output. go: apply everything pending at once (changes live output; needs confirm: true). clear: drop everything pending.",
		inputSchema: {
			type: "object",
			properties: { action: { type: "string", enum: ["enter", "go", "clear"] }, ...confirmProperty },
			required: ["action"],
		},
		async run(backend, input) {
			const desk = controlDesk(backend);
			if (input.action === "clear") return commandResult(await pressKey(desk, "PRE", true));
			const [programmer] = await desk.request<Array<{ preload_pending: unknown[]; preload_group_pending: Record<string, unknown>; preload_capture_programmer: boolean; blind: boolean }>>("GET", "/api/v2/programmers");
			const pending = Boolean(programmer && (programmer.preload_pending.length || Object.keys(programmer.preload_group_pending).length));
			if (input.action === "go") {
				if (input.confirm !== true) throw new Error("Preload GO changes live output. Call again with confirm: true.");
				return commandResult(await pressKey(desk, "PRE"));
			}
			if (pending) return { outcome: "accepted", action: "already_in_preload", command_line: (await desk.request<{ text: string }>("GET", "/api/v2/command-line")).text };
			return commandResult(await pressKey(desk, "PRE"));
		},
	},
	{
		name: "snapshot",
		description:
			"A top-view picture of the rig as it currently looks: each fixture at its stage position, coloured and brightened by its output (or its pending Preload with preload: true). A running fade shows its current point. Use it to check a look before recording it.",
		inputSchema: { type: "object", properties: { preload: { type: "boolean" } } },
		async run(backend, input) {
			const desk = controlDesk(backend);
			const showId = await activeShowId(desk);
			const [patch, frame] = await Promise.all([
				desk.patch(),
				desk.request<{ profile_output_values?: Array<{ fixture_id: string; attribute: string; value: any }>; values?: Array<{ fixture_id: string; attribute: string; value: any }> }>(
					"GET",
					`/api/v2/output/visualization${input.preload ? "?preload=true" : ""}`,
					undefined,
					true,
					undefined,
					{ showId },
				),
			]);
			const { png, summary } = snapshotImage(patch.fixtures, frame.profile_output_values ?? frame.values ?? []);
			return mcpContent([
				{ type: "image", data: png, mimeType: "image/png" },
				{
					type: "text",
					text: `Top view, upstage at the top. Fill colour is each fixture's output colour; brightness is its intensity.\n${JSON.stringify(summary, null, 2)}`,
				},
			]);
		},
	},
];

export const programmingTools: Tool[] = [...readTools, ...actTools];
