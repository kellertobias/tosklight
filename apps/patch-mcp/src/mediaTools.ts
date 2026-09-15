/**
 * The Architect's media layout, as tools: media servers, the sources they advertise, LED module
 * types, the surfaces a source is shown on, and the projectors that light them.
 *
 * Every write reads the layout first and sends one whole object back against the revision it read,
 * so an object someone changed in a window in between is refused rather than overwritten. An
 * update merges the fields given over the stored object; nothing else about it changes.
 *
 * Names follow the editor's own fields (`crates/viz/document/src/media.rs`) in snake_case. The
 * editor stores most of them in camelCase but a surface section's kind fields in snake_case, so the
 * conversion here keeps those as they are.
 */

import {
	type MediaEntryWire,
	type MediaLayoutWire,
	type PatchBackend,
	UnsupportedByProduct,
} from "./backend";
import type { Tool } from "./toolSchema";

const COLLECTIONS: Record<string, string> = {
	media_fallback_asset: "fallbackAssets",
	media_server: "servers",
	media_source: "sources",
	led_module_type: "ledModuleTypes",
	media_surface: "surfaces",
	media_projector: "projectors",
};
const EDITABLE_KINDS = [
	"media_server",
	"media_source",
	"led_module_type",
	"media_surface",
	"media_projector",
];
/** Keys the editor stores in snake_case, whose values are passed through untouched. */
const VERBATIM = new Set(["material", "edge_feather", "bezel_metres", "module_type_id", "occupied_cells"]);

function toWire(value: unknown): unknown {
	if (Array.isArray(value)) return value.map(toWire);
	if (value === null || typeof value !== "object") return value;
	return Object.fromEntries(
		Object.entries(value as Record<string, unknown>).map(([key, inner]) =>
			VERBATIM.has(key)
				? [key, inner]
				: [key.replace(/_([a-z0-9])/g, (_, character: string) => character.toUpperCase()), toWire(inner)],
		),
	);
}

function fromWire(value: unknown): unknown {
	if (Array.isArray(value)) return value.map(fromWire);
	if (value === null || typeof value !== "object") return value;
	return Object.fromEntries(
		Object.entries(value as Record<string, unknown>).map(([key, inner]) => [
			key.replace(/[A-Z]/g, (character) => `_${character.toLowerCase()}`),
			fromWire(inner),
		]),
	);
}

function media(desk: PatchBackend) {
	if (!desk.mediaLayout || !desk.applyMediaIntent)
		throw new UnsupportedByProduct(
			desk.product,
			"plan a media layout; media servers, surfaces, LED walls and projectors are planned in the Architect",
		);
	return {
		layout: () => desk.mediaLayout?.() as Promise<MediaLayoutWire>,
		apply: (kind: string, id: string, intent: Record<string, unknown>) =>
			desk.applyMediaIntent?.(kind, id, intent) as ReturnType<
				NonNullable<PatchBackend["applyMediaIntent"]>
			>,
	};
}

const entryOf = (layout: MediaLayoutWire, kind: string, id: string) =>
	(layout[COLLECTIONS[kind]] ?? []).find((entry) => entry.object.body.id === id);

const readable = (entry: MediaEntryWire) => ({
	revision: entry.revision,
	...(fromWire(entry.object.body) as Record<string, unknown>),
});

/** Only the fields a call actually gave, in the editor's spelling. */
function given(input: Record<string, any>, fields: string[]) {
	const picked = Object.fromEntries(
		fields.filter((field) => input[field] !== undefined).map((field) => [field, input[field]]),
	);
	return toWire(picked) as Record<string, unknown>;
}

/**
 * Create or update one media object.
 *
 * `build` returns the fields to write over the stored object; `required` names what a new one
 * cannot be created without, so the refusal names the missing field rather than a parse error.
 */
async function putMedia(
	desk: PatchBackend,
	kind: string,
	input: Record<string, any>,
	required: string[],
	build: (existing: Record<string, any> | undefined) => Record<string, unknown>,
) {
	const editor = media(desk);
	const layout = await editor.layout();
	const id = String(input.id ?? crypto.randomUUID());
	const existing = entryOf(layout, kind, id);
	const missing = required.filter((field) => input[field] === undefined);
	if (!existing && missing.length > 0)
		throw new Error(`a new ${kind} needs ${missing.join(", ")}`);
	const body = { ...(existing?.object.body ?? {}), ...build(existing?.object.body), id };
	const outcome = await editor.apply(kind, id, {
		requestId: crypto.randomUUID(),
		expectedRevision: existing?.revision ?? 0,
		action: { type: "put", object: { kind, body } },
	});
	const saved = entryOf(outcome.snapshot, kind, id);
	return { kind, created: !existing, ...(saved ? readable(saved) : { id }) };
}

const vector3 = {
	type: "array",
	items: { type: "number" },
	minItems: 3,
	maxItems: 3,
};

function transformOf(input: Record<string, any>, existing?: Record<string, any>) {
	return {
		positionMetres: input.position_metres ?? existing?.positionMetres ?? [0, 0, 0],
		rotationDegrees: input.rotation_degrees ?? existing?.rotationDegrees ?? [0, 0, 0],
	};
}

/** One surface section as the editor stores it, with the Media workspace's own defaults. */
function sectionWire(section: Record<string, any>, index: number) {
	const base = {
		id: section.id ?? crypto.randomUUID(),
		name: section.name ?? `Section ${index + 1}`,
		transform: transformOf(section),
		widthMetres: section.width_metres,
		heightMetres: section.height_metres,
		crop: { left: 0, top: 0, width: 1, height: 1, ...(section.crop ?? {}) },
		type: section.type,
	};
	switch (section.type) {
		case "projection_screen":
			return {
				...base,
				material: section.material ?? { type: "white" },
				edge_feather: section.edge_feather ?? 0,
			};
		case "tv":
			return { ...base, bezel_metres: section.bezel_metres ?? 0, spill: section.spill ?? 0 };
		case "led": {
			const cells = (Number(section.rows) || 0) * (Number(section.columns) || 0);
			return {
				...base,
				module_type_id: section.module_type_id,
				rows: section.rows,
				columns: section.columns,
				occupied_cells: section.occupied_cells ?? Array.from({ length: cells }, (_, cell) => cell),
			};
		}
		default:
			throw new Error(`section ${index + 1}: type must be projection_screen, tv or led`);
	}
}

const sectionSchema = {
	type: "object",
	properties: {
		id: { type: "string", description: "Kept when editing a section; omitted for a new one." },
		name: { type: "string" },
		type: { type: "string", enum: ["projection_screen", "tv", "led"] },
		width_metres: { type: "number" },
		height_metres: { type: "number" },
		position_metres: { ...vector3, description: "[x, y, z] relative to the surface." },
		rotation_degrees: vector3,
		crop: {
			type: "object",
			description: "The part of the source shown, as 0..1 fractions. Whole image when omitted.",
			properties: {
				left: { type: "number" },
				top: { type: "number" },
				width: { type: "number" },
				height: { type: "number" },
			},
		},
		material: {
			type: "object",
			description: "projection_screen only. {type: white | grey_home_cinema} or {type: custom, gain, tint_srgb, roughness}.",
		},
		edge_feather: { type: "number", description: "projection_screen only." },
		bezel_metres: { type: "number", description: "tv only." },
		spill: { type: "number", description: "tv only." },
		module_type_id: { type: "string", description: "led only: an LED module type id." },
		rows: { type: "number", description: "led only." },
		columns: { type: "number", description: "led only." },
		occupied_cells: {
			type: "array",
			items: { type: "number" },
			description: "led only: row-major cell indices that hold a module. Every cell when omitted.",
		},
	},
	required: ["type", "width_metres", "height_metres"],
};

const idInput = {
	type: "string",
	description: "The object's id. Omit to create a new one; give an existing id to update it.",
};

export const mediaTools: Tool[] = [
	{
		name: "get_media_layout",
		description:
			"The Architect's media layout: media servers, their sources, LED module types, surfaces with their sections, and projectors, each with its id and revision.",
		inputSchema: { type: "object", properties: {} },
		async run(desk) {
			const layout = await media(desk).layout();
			const list = (kind: string) => (layout[COLLECTIONS[kind]] ?? []).map(readable);
			return {
				servers: list("media_server"),
				sources: list("media_source"),
				led_module_types: list("led_module_type"),
				surfaces: list("media_surface"),
				projectors: list("media_projector"),
				// The image bytes are the show's own copy and no use to a tool; what it is, is.
				fallback_assets: (layout.fallbackAssets ?? []).map((entry) => {
					const body = entry.object.body;
					return {
						revision: entry.revision,
						id: body.id,
						name: body.name,
						media_type: body.mediaType,
						width: body.width,
						height: body.height,
					};
				}),
			};
		},
	},
	{
		name: "put_media_server",
		description: "Create or update a media server the show sends video to, reached over CITP.",
		inputSchema: {
			type: "object",
			properties: {
				id: idInput,
				name: { type: "string" },
				citp_host: { type: "string", description: "Defaults to 127.0.0.1." },
				citp_port: { type: "number", description: "Defaults to 4809." },
				discovery_identity: { type: "string" },
				last_known_endpoint: { type: "string" },
			},
		},
		run: (desk, input) =>
			putMedia(desk, "media_server", input, ["name"], (existing) => {
				const fields = given(input, ["name", "last_known_endpoint"]);
				const citp = given(
					{ host: input.citp_host, port: input.citp_port, discovery_identity: input.discovery_identity },
					["host", "port", "discovery_identity"],
				);
				if (Object.keys(citp).length > 0) fields.citp = { ...(existing?.citp ?? {}), ...citp };
				return fields;
			}),
	},
	{
		name: "put_media_source",
		description:
			"Create or update a source a media server advertises. The numeric advertised source id is what identifies it on the server.",
		inputSchema: {
			type: "object",
			properties: {
				id: idInput,
				server_id: { type: "string" },
				advertised_source_id: { type: "number" },
				name: { type: "string" },
				output_name: { type: "string" },
				width: { type: "number", description: "Pixels." },
				height: { type: "number", description: "Pixels." },
				aspect_ratio: { type: "number" },
			},
		},
		run: (desk, input) =>
			putMedia(desk, "media_source", input, ["server_id", "advertised_source_id", "name"], () =>
				given(input, [
					"server_id",
					"advertised_source_id",
					"name",
					"output_name",
					"width",
					"height",
					"aspect_ratio",
				]),
			),
	},
	{
		name: "put_led_module_type",
		description: "Create or update an LED module type that LED sections are built from.",
		inputSchema: {
			type: "object",
			properties: {
				id: idInput,
				name: { type: "string" },
				width_metres: { type: "number" },
				height_metres: { type: "number" },
				pixel_pitch_millimetres: { type: "number" },
				horizontal_gap_metres: { type: "number" },
				vertical_gap_metres: { type: "number" },
				pixel_width: { type: "number" },
				pixel_height: { type: "number" },
			},
		},
		run: (desk, input) =>
			putMedia(
				desk,
				"led_module_type",
				input,
				["name", "width_metres", "height_metres", "pixel_pitch_millimetres", "pixel_width", "pixel_height"],
				() =>
					given(input, [
						"name",
						"width_metres",
						"height_metres",
						"pixel_pitch_millimetres",
						"horizontal_gap_metres",
						"vertical_gap_metres",
						"pixel_width",
						"pixel_height",
					]),
			),
	},
	{
		name: "put_media_surface",
		description:
			"Create or update a surface a source is shown on, made of projection-screen, TV and LED sections. Sections, when given, replace the surface's sections.",
		inputSchema: {
			type: "object",
			properties: {
				id: idInput,
				name: { type: "string" },
				source_id: { type: ["string", "null"], description: "The source shown. Null shows none." },
				sections: { type: "array", items: sectionSchema },
			},
		},
		run: (desk, input) =>
			putMedia(desk, "media_surface", input, ["name"], () => {
				const fields = given(input, ["name", "source_id"]);
				if (input.sections !== undefined)
					fields.sections = (input.sections as Array<Record<string, any>>).map(sectionWire);
				return fields;
			}),
	},
	{
		name: "put_media_projector",
		description: "Create or update a projector that lights a surface.",
		inputSchema: {
			type: "object",
			properties: {
				id: idInput,
				name: { type: "string" },
				surface_id: { type: "string" },
				position_metres: vector3,
				rotation_degrees: vector3,
				body_model: { type: "string", description: "Defaults to `projector`." },
				throw_ratio: { type: "number", description: "Defaults to 1.5." },
				lens_shift: { type: "array", items: { type: "number" }, minItems: 2, maxItems: 2 },
				cone_length_metres: { type: "number", description: "Defaults to 12." },
				spill: { type: "number" },
			},
		},
		run: (desk, input) =>
			putMedia(desk, "media_projector", input, ["name", "surface_id"], (existing) => {
				const fields = given(input, [
					"name",
					"surface_id",
					"body_model",
					"throw_ratio",
					"lens_shift",
					"cone_length_metres",
					"spill",
				]);
				if (!existing) Object.assign(fields, { bodyModel: "projector", throwRatio: 1.5, lensShift: [0, 0], coneLengthMetres: 12, ...fields });
				if (!existing || input.position_metres || input.rotation_degrees)
					fields.transform = transformOf(input, existing?.transform);
				return fields;
			}),
	},
	{
		name: "delete_media_object",
		description:
			"Delete one media layout object. Deleting a server removes its sources too; a surface that showed one keeps its fallback image.",
		inputSchema: {
			type: "object",
			properties: {
				kind: { type: "string", enum: EDITABLE_KINDS },
				id: { type: "string" },
			},
			required: ["kind", "id"],
		},
		async run(desk, input) {
			if (!EDITABLE_KINDS.includes(input.kind))
				throw new Error(`kind must be one of ${EDITABLE_KINDS.join(", ")}`);
			const editor = media(desk);
			const existing = entryOf(await editor.layout(), input.kind, input.id);
			if (!existing) throw new Error(`no ${input.kind} with id ${input.id}`);
			const outcome = await editor.apply(input.kind, input.id, {
				requestId: crypto.randomUUID(),
				expectedRevision: existing.revision,
				action: { type: "delete", kind: input.kind, id: input.id },
			});
			return {
				kind: input.kind,
				id: input.id,
				deleted: !entryOf(outcome.snapshot, input.kind, input.id),
			};
		},
	},
];
