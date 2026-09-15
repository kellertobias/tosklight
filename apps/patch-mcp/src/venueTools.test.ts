import { describe, expect, it } from "vitest";
import { memoryBackend } from "./testing/memoryBackend";
import { tools } from "./tools";

const tool = (name: string) => {
	const found = tools.find((candidate) => candidate.name === name);
	if (!found) throw new Error(`missing tool ${name}`);
	return found;
};

const curtain = (revision: number) => ({
	id: "curtain",
	revision,
	manufacturer: "Generic",
	name: "Curtain",
	patch_policy: "visual_only",
	modes: [{ id: `curtain-mode-${revision}`, name: "Curtain" }],
	scenery: {
		kind: "curtain",
		default_size_metres: { x: 6, y: 4, z: 0.1 },
		minimum_size_metres: { x: 0.5, y: 0.5, z: 0.05 },
		maximum_size_metres: { x: 30, y: 20, z: 1 },
		adjustable: { width: true, height: true, depth: false },
	},
});

const chain = {
	id: "chain",
	revision: 1,
	manufacturer: "Generic",
	name: "Chain",
	patch_policy: "visual_only",
	modes: [{ id: "chain-mode", name: "Chain" }],
	scenery: {
		kind: "chain",
		default_size_metres: { x: 0.05, y: 2, z: 0.05 },
		minimum_size_metres: { x: 0.05, y: 0.2, z: 0.05 },
		maximum_size_metres: { x: 0.05, y: 30, z: 0.05 },
		adjustable: { width: false, height: true, depth: false },
	},
};

const dimmer = {
	id: "dimmer",
	revision: 1,
	manufacturer: "Generic",
	name: "Dimmer",
	patch_policy: "dmx",
	modes: [{ id: "dm", name: "1ch" }],
};

const profiles = [curtain(1), curtain(2), chain, dimmer];

describe("venue tools", () => {
	it("adds a curtain by name at its newest revision, in the 0.N namespace, sized and coloured", async () => {
		const { backend, writes } = memoryBackend({
			profiles,
			fixtures: [{ virtual_fixture_number: 2, fixture_number: null, profile_id: "chain" }],
		});

		const added = (await tool("add_fixture").run(backend, {
			profile_name: "curtain",
			size_metres: { width: 12, height: 7.5 },
			colour: "#aa0000",
		})) as any;

		const sent = writes[0].fixtures[0] as any;
		expect(writes[0].revision).toBe(3);
		expect(sent).toMatchObject({
			fixture_number: null,
			virtual_fixture_number: 3,
			profile_revision: 2,
			mode_id: "curtain-mode-2",
			name: "Curtain",
			// Stored in millimetres like every other measurement; the depth stays at its default.
			scenery_size_metres: { x: 12_000, y: 7_500, z: 100 },
			scenery_options: { colour_srgb: "#AA0000" },
		});
		// A Venue object has no DMX address.
		expect(sent.split_patches).toEqual([{ split: 1, universe: null, address: null }]);
		expect(added).toMatchObject({
			id: "0.3",
			venue: { kind: "curtain", size_metres: { width: 12, height: 7.5, depth: 0.1 } },
		});
	});

	it("refuses a size outside the profile's range, or on a measurement the profile fixes", async () => {
		const { backend, writes } = memoryBackend({ profiles });

		await expect(
			tool("add_fixture").run(backend, { profile_name: "Curtain", size_metres: { width: 40 } }),
		).rejects.toThrow("enter a width from 0.5 to 30 metres");
		await expect(
			tool("add_fixture").run(backend, { profile_name: "Curtain", size_metres: { depth: 0.5 } }),
		).rejects.toThrow("depth is fixed by its profile; only width, height can be set");
		expect(writes).toHaveLength(0);
	});

	it("keeps DMX numbering and Venue numbering apart", async () => {
		const { backend, writes } = memoryBackend({ profiles, fixtures: [{}, {}] });

		await expect(
			tool("add_fixture").run(backend, { profile_name: "Curtain", universe: 1, address: 1 }),
		).rejects.toThrow("takes no DMX address");
		await expect(
			tool("add_fixture").run(backend, { profile_name: "Curtain", fixture_number: 5 }),
		).rejects.toThrow("takes a 0.N number, not 5");
		await expect(
			tool("add_fixture").run(backend, { profile_name: "Dimmer", fixture_number: "0.4" }),
		).rejects.toThrow("only a visual-only Venue object takes a 0.N number");
		await expect(
			tool("add_fixture").run(backend, { profile_name: "Dimmer", fixture_number: 2 }),
		).rejects.toThrow("fixture 2 is already in use");
		await expect(
			tool("add_fixture").run(backend, { profile_name: "Dimmer", colour: "#000000" }),
		).rejects.toThrow("is not a Venue object");

		const added = (await tool("add_fixture").run(backend, {
			profile_name: "Dimmer",
			universe: 2,
			address: 10,
		})) as any;
		expect(added).toMatchObject({ id: "3", fixture_number: 3, venue: null });
		expect(writes[0].fixtures[0].split_patches).toEqual([{ split: 1, universe: 2, address: 10 }]);
	});

	it("names a profile two makers share only with its manufacturer", async () => {
		const { backend } = memoryBackend({
			profiles: [...profiles, { ...curtain(1), id: "other", manufacturer: "Acme" }],
		});
		await expect(tool("add_fixture").run(backend, { profile_name: "Curtain" })).rejects.toThrow(
			"2 profiles are named Curtain",
		);
		const added = (await tool("add_fixture").run(backend, {
			profile_name: "Curtain",
			manufacturer: "acme",
		})) as any;
		expect(added.profile_id).toBe("other");
	});

	it("sets one measurement of a placed Venue object by its 0.N number", async () => {
		const { backend, writes } = memoryBackend({
			profiles,
			fixtures: [
				{
					fixture_number: null,
					virtual_fixture_number: 1,
					profile_id: "curtain",
					profile_revision: 2,
					scenery_size_metres: { x: 8_000, y: 5_000, z: 100 },
				},
			],
		});

		const result = (await tool("set_venue_size").run(backend, {
			fixture_number: "0.1",
			height: 6,
		})) as any;

		expect(writes[0].fixtures[0].scenery_size_metres).toEqual({ x: 8_000, y: 6_000, z: 100 });
		expect(result).toMatchObject({ id: "0.1", size_metres: { width: 8, height: 6 } });
		await expect(
			tool("set_venue_size").run(backend, { fixture_number: 0.1, height: 25 }),
		).rejects.toThrow("from 0.5 to 20 metres");
	});

	it("dresses a chain's ends and resets a colour, and refuses chain ends on a curtain", async () => {
		const { backend, writes } = memoryBackend({
			profiles,
			fixtures: [
				{
					fixture_number: null,
					virtual_fixture_number: 1,
					profile_id: "chain",
					scenery_options: { colour_srgb: "#111111" },
				},
				{ fixture_number: null, virtual_fixture_number: 2, profile_id: "curtain", profile_revision: 2 },
				{ fixture_number: 7 },
			],
		});

		await tool("set_venue_options").run(backend, { fixture_number: "0.1", chain_mode: "motor_bottom" });
		expect(writes[0].fixtures[0].scenery_options).toEqual({
			colour_srgb: "#111111",
			chain_top: "steelflex_loop",
			chain_bottom: "motor",
		});
		await tool("set_venue_options").run(backend, { fixture_number: "0.1", colour: null, chain_top: "direct" });
		expect(writes[1].fixtures[0].scenery_options).toEqual({ chain_top: "direct", chain_bottom: "motor" });

		await expect(
			tool("set_venue_options").run(backend, { fixture_number: "0.2", chain_top: "motor" }),
		).rejects.toThrow("chain ends only apply to a chain; this is a curtain");
		await expect(
			tool("set_venue_options").run(backend, { fixture_number: "0.2", colour: "red" }),
		).rejects.toThrow("must be #RRGGBB");
		await expect(
			tool("set_venue_options").run(backend, { fixture_number: 7, colour: "#FFFFFF" }),
		).rejects.toThrow("is not a Venue object");
	});

	it("scales a model within 0.01 to 100 and resets it with null", async () => {
		const { backend, writes } = memoryBackend({ profiles, fixtures: [{}] });
		await tool("set_model_scale").run(backend, { fixture_number: 1, model_scale: 2.5 });
		await tool("set_model_scale").run(backend, { fixture_number: 1, model_scale: null });
		expect(writes.map((write) => write.fixtures[0].model_scale)).toEqual([2.5, null]);
		await expect(
			tool("set_model_scale").run(backend, { fixture_number: 1, model_scale: 0 }),
		).rejects.toThrow("must be from 0.01 to 100");
	});

	it("lists what was placed: Venue kind, size in metres, colour, chain ends and scale", async () => {
		const { backend } = memoryBackend({
			profiles,
			fixtures: [
				{
					fixture_number: null,
					virtual_fixture_number: 4,
					profile_id: "chain",
					scenery_options: { chain_top: "motor" },
					model_scale: 1.5,
				},
				{},
			],
		});

		const listed = (await tool("list_fixtures").run(backend, {})) as any[];

		expect(listed[0]).toMatchObject({
			id: "0.4",
			virtual_fixture_number: 4,
			profile: "Generic Chain",
			model_scale: 1.5,
			venue: {
				kind: "chain",
				size_metres: { width: 0.05, height: 2, depth: 0.05 },
				adjustable: ["height"],
				colour: null,
				chain_top: "motor",
				chain_bottom: null,
			},
		});
		expect(listed[1]).toMatchObject({ id: "2", profile: "Generic Dimmer", venue: null });
	});

	it("removes a Venue object by its 0.N number", async () => {
		const { backend, writes } = memoryBackend({
			profiles,
			fixtures: [{ fixture_number: null, virtual_fixture_number: 3, profile_id: "curtain" }],
		});
		await tool("remove_fixture").run(backend, { fixture_number: "0.3" });
		expect(writes[0].removed).toEqual(["id-1"]);
	});
});
