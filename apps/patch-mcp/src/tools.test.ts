import { describe, expect, it } from "vitest";
import { Desk, type PatchedFixture } from "./desk";
import { tools } from "./tools";

/** A desk that answers from memory and records what was written to it. */
function fakeDesk(
	fixtures: Partial<PatchedFixture>[],
	layers: Array<{ id: string; revision: number; name: string; order: number }> = [],
) {
	const state = {
		patch_revision: 7,
		fixtures: fixtures.map((fixture, index) => ({
			fixture_id: `id-${index + 1}`,
			fixture_number: index + 1,
			name: `Fixture ${index + 1}`,
			profile_id: "profile",
			profile_revision: 1,
			mode_id: "mode",
			layer_id: "default",
			split_patches: [{ split: 1, universe: 1, address: index + 1 }],
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			multipatch: [],
			invert_pan: false,
			invert_tilt: false,
			bracket_angle: 0,
			shaper_angle: null,
			installed_appearance: {},
			...fixture,
		})) as PatchedFixture[],
	};
	const written: Array<{ revision: string | null; body: any }> = [];
	const layerSaves: Array<{ path: string; body: any }> = [];
	const desk = new Desk({
		baseUrl: "http://desk",
		deskId: "test",
		fetch: (async (url: string, init: any) => {
			const path = url.replace("http://desk", "");
			if (path === "/api/v2/sessions") {
				return new Response(JSON.stringify({ token: "t" }), { status: 200 });
			}
			if (path === "/api/v2/patch" && (!init || init.method === "GET")) {
				return new Response(JSON.stringify(state), { status: 200 });
			}
			if (path === "/api/v2/patch/fixtures") {
				written.push({
					revision: init.headers["if-match"] ?? null,
					body: JSON.parse(init.body),
				});
				return new Response("", { status: 200 });
			}
			if (path === "/api/v2/objects/patch_layer") {
				return new Response(
					JSON.stringify({
						objects: layers.map((layer) => ({
							id: layer.id,
							revision: layer.revision,
							body: { name: layer.name, order: layer.order },
						})),
					}),
					{ status: 200 },
				);
			}
			if (path.startsWith("/api/v2/patch/layers/")) {
				layerSaves.push({ path, body: JSON.parse(init.body) });
				return new Response("", { status: 200 });
			}
			if (path === "/api/v2/fixture-library/profiles") {
				return new Response(
					JSON.stringify({
						profiles: [
							{
								id: "p1",
								revision: 3,
								manufacturer: "ToskLight",
								name: "3D Point",
								modes: [{ id: "m1", name: "3D Point", splits: [{ footprint: 0 }] }],
							},
							{
								id: "p2",
								revision: 1,
								manufacturer: "Generic",
								name: "Dimmer",
								modes: [{ id: "m2", name: "1ch", splits: [{ footprint: 1 }] }],
							},
						],
					}),
					{ status: 200 },
				);
			}
			throw new Error(`unexpected ${path}`);
		}) as unknown as typeof fetch,
	});
	return { desk, written, layerSaves, state };
}

const tool = (name: string) => {
	const found = tools.find((candidate) => candidate.name === name);
	if (!found) throw new Error(`missing tool ${name}`);
	return found;
};

describe("patch tools", () => {
	it("lists the layers the show stores, including one nothing is patched onto", async () => {
		const { desk } = fakeDesk(
			[{ layer_id: "truss" }, { layer_id: "truss" }],
			[
				{ id: "floor", revision: 2, name: "Floor", order: 1 },
				{ id: "truss", revision: 3, name: "Front truss", order: 0 },
			],
		);

		const listed = (await tool("list_layers").run(desk, {})) as any[];

		// Sorted by the show's own order, not by name or by how many fixtures stand on them.
		expect(listed.map((layer) => layer.layer_id)).toEqual(["truss", "floor"]);
		expect(listed[0]).toMatchObject({ name: "Front truss", fixtures: 2 });
		// A named layer with nothing on it is still a layer.
		expect(listed[1]).toMatchObject({ name: "Floor", fixtures: 0 });
	});

	it("reports a layer that fixtures name but the show has no record for", async () => {
		const { desk } = fakeDesk([{ layer_id: "orphan" }], []);

		const listed = (await tool("list_layers").run(desk, {})) as any[];

		expect(listed).toEqual([
			{ layer_id: "orphan", name: "orphan", order: 0, fixtures: 1 },
		]);
	});

	it("creates a layer at the end and saves an edit against the revision it read", async () => {
		const { desk, layerSaves } = fakeDesk(
			[],
			[{ id: "floor", revision: 2, name: "Floor", order: 4 }],
		);

		await tool("save_layer").run(desk, { layer_id: "truss", name: "Truss" });
		await tool("save_layer").run(desk, { layer_id: "floor", name: "Stage floor" });

		// A new layer lands past the last one rather than on top of whatever holds order zero.
		expect(layerSaves[0].body.action).toMatchObject({
			expected_revision: 0,
			layer: { name: "Truss", order: 5 },
		});
		// An edit keeps where the layer already sits, and is written against its own revision.
		expect(layerSaves[1].body.action).toMatchObject({
			expected_revision: 2,
			layer: { name: "Stage floor", order: 4 },
		});
	});

	it("empties a layer by moving its fixtures, and refuses to empty one onto itself", async () => {
		const { desk, written } = fakeDesk(
			[{ layer_id: "truss" }, { layer_id: "floor" }, { layer_id: "truss" }],
			[
				{ id: "truss", revision: 1, name: "Truss", order: 0 },
				{ id: "floor", revision: 1, name: "Floor", order: 1 },
			],
		);

		const result = (await tool("remove_layer").run(desk, {
			layer_id: "truss",
			move_to_layer_id: "floor",
		})) as any;

		expect(result.moved_fixtures).toEqual([1, 3]);
		// Only the fixtures that were on the emptied layer are written, and against the read revision.
		expect(written[0].revision).toBe("7");
		expect(written[0].body.fixtures.map((f: any) => f.layer_id)).toEqual([
			"floor",
			"floor",
		]);
		expect(written[0].body.remove_fixture_ids).toEqual([]);

		await expect(
			tool("remove_layer").run(desk, {
				layer_id: "truss",
				move_to_layer_id: "truss",
			}),
		).rejects.toThrow("cannot be emptied onto itself");
	});

	it("searches the library by manufacturer, name or mode", async () => {
		const { desk } = fakeDesk([]);
		const found = (await tool("search_fixture_library").run(desk, {
			query: "point",
		})) as any[];
		expect(found).toHaveLength(1);
		expect(found[0].name).toBe("3D Point");
		// Adding a fixture needs all three of these, so the search has to return them.
		expect(found[0].profile_id).toBe("p1");
		expect(found[0].profile_revision).toBe(3);
		expect(found[0].modes[0].mode_id).toBe("m1");
	});

	it("changes only the field it was asked to change", async () => {
		const { desk, written } = fakeDesk([
			{ name: "Front left", location: { x: 100, y: 200, z: 300 } },
		]);
		await tool("set_fixture_placement").run(desk, {
			fixture_number: 1,
			z: 4000,
		});
		const sent = written[0].body.fixtures[0];
		// The one field asked for moved; the rest of the placement, and the name, did not.
		expect(sent.location).toEqual({ x: 100, y: 200, z: 4000 });
		expect(sent.name).toBe("Front left");
		expect(sent.split_patches).toEqual([{ split: 1, universe: 1, address: 1 }]);
	});

	it("writes against the revision it read, so a moved patch is refused rather than overwritten", async () => {
		const { desk, written } = fakeDesk([{}]);
		await tool("set_fixture_identity").run(desk, {
			fixture_number: 1,
			name: "Renamed",
		});
		expect(written[0].revision).toBe("7");
	});

	it("sets a name and a note without touching the other", async () => {
		const { desk, written } = fakeDesk([{ name: "Old", note: "circuit 12" }]);
		await tool("set_fixture_identity").run(desk, {
			fixture_number: 1,
			name: "New",
		});
		const sent = written[0].body.fixtures[0];
		expect(sent.name).toBe("New");
		expect(sent.note).toBe("circuit 12");
	});

	it("unpatches a fixture without removing it from the show", async () => {
		const { desk, written } = fakeDesk([{}]);
		await tool("set_fixture_patch").run(desk, {
			fixture_number: 1,
			universe: null,
			address: null,
		});
		const sent = written[0].body.fixtures[0];
		expect(sent.split_patches[0]).toEqual({
			split: 1,
			universe: null,
			address: null,
		});
		// Still in the show: an unpatched fixture is selectable and programmable.
		expect(written[0].body.remove_fixture_ids).toEqual([]);
	});

	it("slaves a fixture to a point by the point's fixture number", async () => {
		const { desk, written } = fakeDesk([{ name: "Truss point" }, { name: "Spot" }]);
		await tool("assign_position_master").run(desk, {
			fixture_number: 2,
			master_fixture_number: 1,
		});
		expect(written[0].body.fixtures[0].position_master).toBe("id-1");
	});

	it("unslaves a fixture when no master is named", async () => {
		const { desk, written } = fakeDesk([
			{ name: "Spot", position_master: "id-9" },
		]);
		await tool("assign_position_master").run(desk, { fixture_number: 1 });
		expect(written[0].body.fixtures[0].position_master).toBeNull();
	});

	it("adds a multi-patch instance beside the ones already there", async () => {
		const { desk, written } = fakeDesk([
			{ multipatch: [{ id: "existing" }] as unknown[] },
		]);
		await tool("add_multipatch").run(desk, {
			fixture_number: 1,
			name: "Segment 2",
		});
		const sent = written[0].body.fixtures[0];
		expect(sent.multipatch).toHaveLength(2);
		expect(sent.multipatch[1].name).toBe("Segment 2");
		// No address: an instance that exists in the visualizer only.
		expect(sent.multipatch[1].universe).toBeNull();
	});

	it("removes a fixture by id rather than by number", async () => {
		const { desk, written } = fakeDesk([{}, {}]);
		await tool("remove_fixture").run(desk, { fixture_number: 2 });
		expect(written[0].body.remove_fixture_ids).toEqual(["id-2"]);
		expect(written[0].body.fixtures).toEqual([]);
	});

	it("says which fixture is missing rather than failing silently", async () => {
		const { desk } = fakeDesk([{}]);
		await expect(
			tool("set_fixture_identity").run(desk, { fixture_number: 9, name: "x" }),
		).rejects.toThrow("no fixture numbered 9");
	});
});
