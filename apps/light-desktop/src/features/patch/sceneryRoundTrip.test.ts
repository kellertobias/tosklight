/**
 * A placed Venue object's size, colour and chain ends through the desk's whole patch path: read from
 * the server's wire snapshot, edited in Show Patch, and written back.
 *
 * The Show Patch control tests stop at the changed fixture the sheet hands to the patch feature, so
 * they cannot see a field the write drops on its way to the wire. These go as far as the request body.
 */
import { describe, expect, it, vi } from "vitest";
import { HttpPatchTransport } from "../../api/PatchTransport";
import { decodePatchSnapshot } from "../../api/patchWire";
import type { PatchedFixture } from "../../api/types";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../../components/setup/fixtureProfileModel";
import {
	changedPatchFixtureCandidate,
	createPatchDefinitionResolver,
	projectionToPatchedFixture,
} from "./model";

const SHOW_ID = "10000000-0000-0000-0000-000000000001";
const PROFILE_ID = "20000000-0000-0000-0000-000000000001";
const MODE_ID = "30000000-0000-0000-0000-000000000001";
const FIXTURE_ID = "40000000-0000-0000-0000-000000000001";
const COPY_ID = "40000000-0000-0000-0000-000000000002";

const appearance = {
	light_source: { type: "profile_default" },
	color_temperature_kelvin: null,
	gel: { type: "open_white" },
	shaper_angles_degrees: [0, 0, 0, 0],
};

function venueDefinition() {
	const profile = blankFixtureProfile();
	profile.id = PROFILE_ID;
	profile.revision = 1;
	profile.manufacturer = "Venue";
	profile.name = "Chain";
	profile.short_name = "Chain";
	profile.fixture_type = "rigging";
	profile.patch_policy = "visual_only";
	profile.modes[0].id = MODE_ID;
	profile.modes[0].splits = [{ number: 1, footprint: 0 }];
	return fixtureDefinitionsFromProfiles([profile])[0];
}

/** One placed chain with a stored size, colour and ends, and one copy of it at its own length. */
function wireSnapshot() {
	return {
		show_id: SHOW_ID,
		show_revision: 1,
		patch_revision: 1,
		cursor: { sequence: 10 },
		fixtures: [
			{
				fixture_id: FIXTURE_ID,
				fixture_revision: 1,
				fixture_number: null,
				virtual_fixture_number: 1,
				name: "Chain SL",
				profile_id: PROFILE_ID,
				profile_revision: 1,
				mode_id: MODE_ID,
				split_patches: [{ split: 1, universe: null, address: null }],
				layer_id: "default",
				direct_control: null,
				location: { x: 0, y: 0, z: 8000 },
				rotation: { x: 0, y: 0, z: 0 },
				scenery_size_metres: { x: 0, y: 0, z: 3000 },
				scenery_options: {
					colour_srgb: "#FF0000",
					chain_top: "direct",
					chain_bottom: "motor",
					// Not a chain's choice, but it must survive every write all the same.
					handrails: "left",
				},
				logical_heads: [],
				multipatch: [
					{
						id: COPY_ID,
						name: "Chain SR",
						split_patches: [{ split: 1, universe: null, address: null }],
						location: { x: 4000, y: 0, z: 8000 },
						rotation: { x: 0, y: 0, z: 0 },
						scenery_size_metres: { x: 0, y: 0, z: 5000 },
						invert_pan: false,
						invert_tilt: false,
						bracket_angle: 0,
						shaper_angle: null,
						installed_appearance: appearance,
					},
				],
				installed_appearance: appearance,
				move_in_black_enabled: false,
				move_in_black_delay_millis: 0,
				highlight_overrides: [],
			},
		],
		profile_revisions: [
			{
				profile_id: PROFILE_ID,
				profile_revision: 1,
				content_digest: "digest",
				manufacturer: "Venue",
				name: "Chain",
				fixture_type: "rigging",
				patch_policy: "visual_only",
				referenced_modes: [
					{ mode_id: MODE_ID, name: "Default", splits: [{ split: 1, footprint: 0 }] },
				],
			},
		],
	};
}

/** The stored chain as Show Patch holds it after reading the server's snapshot. */
function storedChain(): PatchedFixture {
	const snapshot = decodePatchSnapshot(wireSnapshot());
	return projectionToPatchedFixture(
		snapshot.fixtures[0],
		snapshot.profileRevisions[0],
		createPatchDefinitionResolver([venueDefinition()]),
	);
}

/** What one write of `fixture` puts on the wire. */
async function writtenFixture(fixture: PatchedFixture) {
	let body: { fixtures: Record<string, unknown>[] } | null = null;
	const fetchMock = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
		body = JSON.parse(String(init?.body));
		const requestId = (body as unknown as { request_id: string }).request_id;
		return new Response(
			JSON.stringify({
				request_id: requestId,
				replayed: false,
				changed: true,
				show_id: SHOW_ID,
				show_revision: 2,
				patch_revision: 2,
				event_sequence: 11,
				fixtures: [],
				removed_fixture_ids: [],
				profile_revisions: [],
			}),
			{ status: 200, headers: { "content-type": "application/json" } },
		);
	});
	const transport = new HttpPatchTransport({
		baseUrl: "http://desk.local",
		sessionToken: "session-token",
		fetch: fetchMock as typeof fetch,
	});
	await transport.patchFixtures(SHOW_ID, 1, {
		requestId: "request-1",
		fixtures: [changedPatchFixtureCandidate(fixture, {}).input],
		removeFixtureIds: [],
	});
	expect(fetchMock).toHaveBeenCalledOnce();
	return (body as { fixtures: Record<string, unknown>[] } | null)?.fixtures[0] ?? {};
}

describe("a Venue object's size, colour and chain ends on the desk", () => {
	it("reads the stored size, colour and chain ends into Show Patch, for the object and its copy", () => {
		const chain = storedChain();
		expect(chain.scenery_size_metres).toEqual({ x: 0, y: 0, z: 3000 });
		expect(chain.scenery_options).toEqual({
			colour_srgb: "#FF0000",
			chain_top: "direct",
			chain_bottom: "motor",
			handrails: "left",
		});
		expect(chain.multipatch?.[0].scenery_size_metres).toEqual({ x: 0, y: 0, z: 5000 });
	});

	it("writes a Footprint, Colour and Chain edit made in Show Patch", async () => {
		const edited = changedPatchFixtureCandidate(storedChain(), {
			scenery_size_metres: { x: 0, y: 0, z: 6500 },
			scenery_options: { colour_srgb: "#00FF00", chain_top: "motor", chain_bottom: "steelflex_loop" },
		}).fixture;
		const written = await writtenFixture(edited);
		expect(written.scenery_size_metres).toEqual({ x: 0, y: 0, z: 6500 });
		expect(written.scenery_options).toEqual({
			colour_srgb: "#00FF00",
			chain_top: "motor",
			chain_bottom: "steelflex_loop",
			handrails: null,
		});
	});

	it("keeps the stored size, colour, chain ends and copy size when only the name is edited", async () => {
		const renamed = changedPatchFixtureCandidate(storedChain(), { name: "Chain DS" }).fixture;
		const written = await writtenFixture(renamed);
		expect(written.name).toBe("Chain DS");
		expect(written.scenery_size_metres).toEqual({ x: 0, y: 0, z: 3000 });
		expect(written.scenery_options).toEqual({
			colour_srgb: "#FF0000",
			chain_top: "direct",
			chain_bottom: "motor",
			handrails: "left",
		});
		expect(
			(written.multipatch as Record<string, unknown>[] | undefined)?.[0]?.scenery_size_metres,
		).toEqual({ x: 0, y: 0, z: 5000 });
	});

	it("writes nothing for an object that was never resized or recoloured", async () => {
		const plain = changedPatchFixtureCandidate(storedChain(), {
			scenery_size_metres: null,
			scenery_options: null,
		}).fixture;
		const written = await writtenFixture(plain);
		expect(written.scenery_size_metres ?? null).toBeNull();
		expect(written.scenery_options ?? null).toBeNull();
	});
});
