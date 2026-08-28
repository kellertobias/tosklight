/**
 * The patch capabilities, as plain functions over a {@link Desk}.
 *
 * Separated from the MCP plumbing on purpose: what these do to a show is worth testing on its own,
 * and a transport is not needed to do it.
 */

import {
	type PatchBackend,
	type PatchedFixture,
	UnsupportedByProduct,
} from "./backend";

export interface Tool {
	name: string;
	description: string;
	inputSchema: {
		type: "object";
		properties: Record<string, unknown>;
		required?: string[];
	};
	run(desk: PatchBackend, input: Record<string, any>): Promise<unknown>;
}

const fixtureNumber = {
	type: "number",
	description: "The fixture number an operator would say out loud.",
};

/** A gel, in the two shapes the patch accepts. */
const gel = {
	type: "object",
	description:
		"Open white when omitted. A built-in gel names a catalog entry; a custom gel carries its own sRGB colour.",
	properties: {
		catalog_id: { type: "string" },
		entry_id: { type: "string" },
		name: { type: "string" },
		color_srgb: {
			type: "string",
			description: "`#rrggbb`, for a custom gel.",
		},
	},
};

function appearanceWithGel(
	fixture: PatchedFixture,
	input: Record<string, any>,
): Record<string, unknown> {
	const appearance = {
		...((fixture.installed_appearance as Record<string, unknown>) ?? {}),
	};
	if (input.gel) {
		appearance.gel = input.gel.catalog_id
			? {
					type: "built_in",
					catalog_id: input.gel.catalog_id,
					entry_id: input.gel.entry_id,
				}
			: {
					type: "custom",
					name: input.gel.name ?? "Custom",
					color_srgb: input.gel.color_srgb,
					note: null,
				};
	}
	return appearance;
}

/** One past the last layer's order, so a new layer lands at the end rather than on top of one. */
async function nextLayerOrder(desk: PatchBackend): Promise<number> {
	const layers = await desk.layers();
	return layers.reduce((highest, layer) => Math.max(highest, layer.order), -1) + 1;
}

export const tools: Tool[] = [
	{
		name: "search_fixture_library",
		description:
			"Search the shipped and imported fixture library. Returns the profile and mode ids that adding a fixture needs.",
		inputSchema: {
			type: "object",
			properties: {
				query: {
					type: "string",
					description:
						"Matched against manufacturer, name and mode. Omit to list everything.",
				},
				limit: { type: "number" },
			},
		},
		async run(desk, input) {
			const { profiles } = await desk.profiles();
			const query = String(input.query ?? "")
				.trim()
				.toLowerCase();
			const matches = profiles.filter((profile) => {
				if (!query) return true;
				const haystack = [
					profile.manufacturer,
					profile.name,
					...((profile.modes as Array<{ name?: string }>) ?? []).map(
						(mode) => mode.name,
					),
				]
					.join(" ")
					.toLowerCase();
				return haystack.includes(query);
			});
			return matches.slice(0, Number(input.limit ?? 25)).map((profile) => ({
				profile_id: profile.id,
				profile_revision: profile.revision,
				manufacturer: profile.manufacturer,
				name: profile.name,
				modes: ((profile.modes as Array<Record<string, unknown>>) ?? []).map(
					(mode) => ({
						mode_id: mode.id,
						name: mode.name,
						footprint: (
							(mode.splits as Array<{ footprint?: number }>) ?? []
						).reduce((total, split) => total + (split.footprint ?? 0), 0),
					}),
				),
			}));
		},
	},
	{
		name: "list_fixtures",
		description:
			"Every fixture in the show, with the fields these tools can change.",
		inputSchema: { type: "object", properties: {} },
		async run(desk) {
			const { fixtures } = await desk.patch();
			return fixtures.map((fixture) => ({
				fixture_number: fixture.fixture_number,
				name: fixture.name,
				note: fixture.note ?? null,
				layer_id: fixture.layer_id,
				location: fixture.location,
				rotation: fixture.rotation,
				bracket_angle: fixture.bracket_angle,
				shaper_angle: fixture.shaper_angle,
				invert_pan: fixture.invert_pan,
				invert_tilt: fixture.invert_tilt,
				position_master: fixture.position_master ?? null,
				split_patches: fixture.split_patches,
				multipatch: (fixture.multipatch as unknown[])?.length ?? 0,
			}));
		},
	},
	{
		name: "add_fixture",
		description:
			"Add a fixture to the show, with its address, placement and gel.",
		inputSchema: {
			type: "object",
			properties: {
				profile_id: { type: "string" },
				profile_revision: { type: "number" },
				mode_id: { type: "string" },
				fixture_number: fixtureNumber,
				name: { type: "string" },
				note: { type: "string" },
				layer_id: { type: "string", description: "Defaults to `default`." },
				universe: { type: "number" },
				address: { type: "number" },
				x: { type: "number", description: "Millimetres across the stage." },
				y: { type: "number", description: "Millimetres upstage." },
				z: { type: "number", description: "Millimetres up." },
				rotation_x: { type: "number" },
				rotation_y: { type: "number" },
				rotation_z: { type: "number" },
				gel,
			},
			required: [
				"profile_id",
				"profile_revision",
				"mode_id",
				"fixture_number",
				"name",
			],
		},
		async run(desk, input) {
			const snapshot = await desk.patch();
			const fixture = {
				fixture_id: crypto.randomUUID(),
				fixture_number: input.fixture_number,
				virtual_fixture_number: null,
				name: input.name,
				note: input.note ?? null,
				profile_id: input.profile_id,
				profile_revision: input.profile_revision,
				mode_id: input.mode_id,
				split_patches: [
					{
						split: 1,
						universe: input.universe ?? null,
						address: input.address ?? null,
					},
				],
				layer_id: input.layer_id ?? "default",
				direct_control: null,
				location: {
					x: input.x ?? 0,
					y: input.y ?? 0,
					z: input.z ?? 0,
				},
				rotation: {
					x: input.rotation_x ?? 0,
					y: input.rotation_y ?? 0,
					z: input.rotation_z ?? 0,
				},
				multipatch: [],
				move_in_black_enabled: false,
				move_in_black_delay_millis: 0,
				highlight_overrides: [],
			} as unknown as PatchedFixture;
			if (input.gel) {
				(fixture as Record<string, unknown>).installed_appearance =
					appearanceWithGel(fixture, input);
			}
			await desk.putFixtures(snapshot.patch_revision, [fixture]);
			return { fixture_number: input.fixture_number, added: true };
		},
	},
	{
		name: "remove_fixture",
		description: "Remove a fixture from the show.",
		inputSchema: {
			type: "object",
			properties: { fixture_number: fixtureNumber },
			required: ["fixture_number"],
		},
		async run(desk, input) {
			const { snapshot, fixture } = await desk.fixture(input.fixture_number);
			await desk.putFixtures(snapshot.patch_revision, [], [fixture.fixture_id]);
			return { fixture_number: input.fixture_number, removed: true };
		},
	},
	{
		name: "set_fixture_placement",
		description:
			"Set a fixture's position, rotation and bracket angle. Omitted values are left alone.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				x: { type: "number" },
				y: { type: "number" },
				z: { type: "number" },
				rotation_x: { type: "number" },
				rotation_y: { type: "number" },
				rotation_z: { type: "number" },
				bracket_angle: {
					type: "number",
					description: "Degrees, positive nose-down.",
				},
			},
			required: ["fixture_number"],
		},
		async run(desk, input) {
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				const location = fixture.location as Record<string, number>;
				const rotation = fixture.rotation as Record<string, number>;
				fixture.location = {
					x: input.x ?? location.x,
					y: input.y ?? location.y,
					z: input.z ?? location.z,
				};
				fixture.rotation = {
					x: input.rotation_x ?? rotation.x,
					y: input.rotation_y ?? rotation.y,
					z: input.rotation_z ?? rotation.z,
				};
				if (input.bracket_angle !== undefined) {
					fixture.bracket_angle = input.bracket_angle;
				}
				return fixture;
			});
			return {
				location: edited.location,
				rotation: edited.rotation,
				bracket_angle: edited.bracket_angle,
			};
		},
	},
	{
		name: "set_fixture_shaper_and_gel",
		description: "Set a fixture's shaper angle and its gel.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				shaper_angle: {
					type: "number",
					description: "Degrees. Null clears a fitted shaper.",
				},
				gel,
			},
			required: ["fixture_number"],
		},
		async run(desk, input) {
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				if ("shaper_angle" in input) fixture.shaper_angle = input.shaper_angle;
				if (input.gel) {
					fixture.installed_appearance = appearanceWithGel(fixture, input);
				}
				return fixture;
			});
			return {
				shaper_angle: edited.shaper_angle,
				installed_appearance: edited.installed_appearance,
			};
		},
	},
	{
		name: "set_fixture_identity",
		description: "Set a fixture's name and its note.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				name: { type: "string" },
				note: {
					type: "string",
					description:
						"The operator's own note for this fixture, not the profile's shared notes.",
				},
			},
			required: ["fixture_number"],
		},
		async run(desk, input) {
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				if (input.name !== undefined) fixture.name = input.name;
				if ("note" in input) fixture.note = input.note;
				return fixture;
			});
			return { name: edited.name, note: edited.note ?? null };
		},
	},
	{
		name: "set_fixture_patch",
		description:
			"Set a fixture's universe and address, and whether its pan or tilt is reversed.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				universe: {
					type: "number",
					description: "Null unpatches the fixture without removing it.",
				},
				address: { type: "number" },
				invert_pan: { type: "boolean" },
				invert_tilt: { type: "boolean" },
			},
			required: ["fixture_number"],
		},
		async run(desk, input) {
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				if ("universe" in input || "address" in input) {
					const splits = fixture.split_patches;
					const first = splits[0] ?? { split: 1, universe: null, address: null };
					splits[0] = {
						split: first.split,
						universe: "universe" in input ? input.universe : first.universe,
						address: "address" in input ? input.address : first.address,
					};
				}
				if (input.invert_pan !== undefined) fixture.invert_pan = input.invert_pan;
				if (input.invert_tilt !== undefined) {
					fixture.invert_tilt = input.invert_tilt;
				}
				return fixture;
			});
			return {
				split_patches: edited.split_patches,
				invert_pan: edited.invert_pan,
				invert_tilt: edited.invert_tilt,
			};
		},
	},
	{
		name: "add_multipatch",
		description:
			"Add another physical instance controlled and selected as this fixture. An instance with no address exists in the visualizer only.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				name: { type: "string" },
				universe: { type: "number" },
				address: { type: "number" },
				x: { type: "number" },
				y: { type: "number" },
				z: { type: "number" },
			},
			required: ["fixture_number", "name"],
		},
		async run(desk, input) {
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				const multipatch = (fixture.multipatch as unknown[]) ?? [];
				multipatch.push({
					id: crypto.randomUUID(),
					name: input.name,
					universe: input.universe ?? null,
					address: input.address ?? null,
					split_patches: [
						{
							split: 1,
							universe: input.universe ?? null,
							address: input.address ?? null,
						},
					],
					location: { x: input.x ?? 0, y: input.y ?? 0, z: input.z ?? 0 },
					rotation: { x: 0, y: 0, z: 0 },
					invert_pan: false,
					invert_tilt: false,
					bracket_angle: 0,
					shaper_angle: null,
					installed_appearance: {},
				});
				fixture.multipatch = multipatch;
				return fixture;
			});
			return { multipatch: (edited.multipatch as unknown[]).length };
		},
	},
	{
		name: "assign_position_master",
		description:
			"Slave a fixture to a 3D Point, so moving or rotating that point carries the fixture with it. Pass no master to unslave it.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				master_fixture_number: {
					type: "number",
					description: "The 3D Point's fixture number. Omit to unslave.",
				},
			},
			required: ["fixture_number"],
		},
		async run(desk, input) {
			let master: string | null = null;
			if (input.master_fixture_number !== undefined) {
				const found = await desk.fixture(input.master_fixture_number);
				master = found.fixture.fixture_id;
			}
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				fixture.position_master = master;
				return fixture;
			});
			return { position_master: edited.position_master ?? null };
		},
	},
	{
		name: "list_layers",
		description:
			"The patch layers the show stores, in their own order, each with the number of fixtures standing on it. A named layer with no fixtures is listed too.",
		inputSchema: { type: "object", properties: {} },
		async run(desk) {
			const [layers, { fixtures }] = await Promise.all([
				desk.layers(),
				desk.patch(),
			]);
			const counts = new Map<string, number>();
			for (const fixture of fixtures) {
				counts.set(fixture.layer_id, (counts.get(fixture.layer_id) ?? 0) + 1);
			}
			const listed = layers.map((layer) => ({
				layer_id: layer.id,
				name: layer.name,
				order: layer.order,
				fixtures: counts.get(layer.id) ?? 0,
			}));
			// A fixture may stand on a layer the show has no record for. Saying so is more use than
			// hiding it: those fixtures are in the patch and an operator has to be able to see them.
			const named = new Set(layers.map((layer) => layer.id));
			for (const [layer_id, fixtures] of counts) {
				if (named.has(layer_id)) continue;
				listed.push({ layer_id, name: layer_id, order: 0, fixtures });
			}
			return listed;
		},
	},
	{
		name: "save_layer",
		description:
			"Create a layer, or rename and reorder one that exists. The id is the layer's stable name in the patch; `name` is what an operator reads and `order` is where it sits in the list.",
		inputSchema: {
			type: "object",
			properties: {
				layer_id: {
					type: "string",
					description: "The layer's stable id. A new id creates a layer.",
				},
				name: { type: "string" },
				order: {
					type: "number",
					description: "Where the layer sits. Lower comes first.",
				},
			},
			required: ["layer_id", "name"],
		},
		async run(desk, input) {
			// The Architect keeps layers on its fixtures and has no route that names or reorders
			// one. Saying which product cannot do this is more use than a failure from inside an
			// HTTP call, and set_fixture_layer still works there.
			if (!desk.saveLayer)
				throw new UnsupportedByProduct(desk.product, "create or rename a layer");
			const existing = await desk.layer(input.layer_id);
			// An unstated order keeps where the layer already sits, and puts a new one at the end
			// rather than on top of whatever happens to hold order zero.
			const order =
				input.order ?? existing?.order ?? (await nextLayerOrder(desk));
			return desk.saveLayer(input.layer_id, input.name, order);
		},
	},
	{
		name: "remove_layer",
		description:
			"Empty a layer by moving every fixture on it to another layer. The desk keeps the layer record itself; there is no route that deletes one.",
		inputSchema: {
			type: "object",
			properties: {
				layer_id: { type: "string" },
				move_to_layer_id: {
					type: "string",
					description: "Where the fixtures go. Must be a different layer.",
				},
			},
			required: ["layer_id", "move_to_layer_id"],
		},
		async run(desk, input) {
			if (input.layer_id === input.move_to_layer_id)
				throw new Error("a layer cannot be emptied onto itself");
			const snapshot = await desk.patch();
			const moved = snapshot.fixtures
				.filter((fixture) => fixture.layer_id === input.layer_id)
				.map((fixture) => ({
					...structuredClone(fixture),
					layer_id: input.move_to_layer_id,
				}));
			if (moved.length > 0)
				await desk.putFixtures(snapshot.patch_revision, moved);
			return {
				layer_id: input.layer_id,
				moved_fixtures: moved.map((fixture) => fixture.fixture_number),
				moved_to: input.move_to_layer_id,
			};
		},
	},
	{
		name: "set_fixture_layer",
		description:
			"Move one fixture onto a layer. The layer should exist already; use save_layer to create it.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				layer_id: { type: "string" },
			},
			required: ["fixture_number", "layer_id"],
		},
		async run(desk, input) {
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				fixture.layer_id = input.layer_id;
				return fixture;
			});
			return { layer_id: edited.layer_id };
		},
	},
];
