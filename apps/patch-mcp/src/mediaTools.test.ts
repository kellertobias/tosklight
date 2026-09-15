import { describe, expect, it } from "vitest";
import { memoryBackend } from "./testing/memoryBackend";
import { tools } from "./tools";

const tool = (name: string) => {
	const found = tools.find((candidate) => candidate.name === name);
	if (!found) throw new Error(`missing tool ${name}`);
	return found;
};

describe("media layout tools", () => {
	it("reads the layout in the tools' spelling, without a fallback image's bytes", async () => {
		const { backend } = memoryBackend({
			media: {
				servers: [
					{
						object: {
							kind: "media_server",
							body: { id: "s1", name: "Pixel", citp: { host: "10.0.0.2", port: 4809 }, lastKnownEndpoint: null },
						},
						revision: 2,
					},
				],
				fallbackAssets: [
					{
						object: {
							kind: "media_fallback_asset",
							body: { id: "f1", name: "Logo", mediaType: "image/png", width: 4, height: 2, bytesBase64: "AAAA" },
						},
						revision: 1,
					},
				],
			},
		});

		const layout = (await tool("get_media_layout").run(backend, {})) as any;

		expect(layout.servers).toEqual([
			{ revision: 2, id: "s1", name: "Pixel", citp: { host: "10.0.0.2", port: 4809 }, last_known_endpoint: null },
		]);
		expect(layout.fallback_assets).toEqual([
			{ revision: 1, id: "f1", name: "Logo", media_type: "image/png", width: 4, height: 2 },
		]);
	});

	it("creates a server, then updates only what it is given against the revision it read", async () => {
		const { backend, intents } = memoryBackend();

		const created = (await tool("put_media_server").run(backend, {
			name: "Pixel",
			citp_host: "10.0.0.5",
			discovery_identity: "pixel-1",
		})) as any;
		expect(intents[0].intent).toMatchObject({
			expectedRevision: 0,
			action: {
				type: "put",
				object: {
					kind: "media_server",
					body: { name: "Pixel", citp: { host: "10.0.0.5", discoveryIdentity: "pixel-1" } },
				},
			},
		});
		expect(intents[0].id).toBe(created.id);
		expect(created).toMatchObject({ created: true, revision: 1 });

		const updated = (await tool("put_media_server").run(backend, { id: created.id, citp_port: 5000 })) as any;
		expect(intents[1].intent.expectedRevision).toBe(1);
		// The name and host stay; the port is merged into the stored CITP settings.
		expect(intents[1].intent.action.object.body).toMatchObject({
			id: created.id,
			name: "Pixel",
			citp: { host: "10.0.0.5", port: 5000, discoveryIdentity: "pixel-1" },
		});
		expect(updated).toMatchObject({ created: false, revision: 2 });
		// Each write carries its own request identity, so a retry is replayed rather than re-applied.
		expect(intents[0].intent.requestId).not.toBe(intents[1].intent.requestId);
	});

	it("says what a new object is missing before sending anything", async () => {
		const { backend, intents } = memoryBackend();
		await expect(tool("put_media_source").run(backend, { name: "Out 1" })).rejects.toThrow(
			"a new media_source needs server_id, advertised_source_id",
		);
		expect(intents).toHaveLength(0);
	});

	it("writes sources and LED module types in the editor's field names", async () => {
		const { backend, intents } = memoryBackend();
		await tool("put_media_source").run(backend, {
			server_id: "s1",
			advertised_source_id: 3,
			name: "Output 3",
			output_name: "HDMI 1",
			width: 1920,
			height: 1080,
		});
		await tool("put_led_module_type").run(backend, {
			name: "ROE CB5",
			width_metres: 0.6,
			height_metres: 0.6,
			pixel_pitch_millimetres: 5.77,
			pixel_width: 104,
			pixel_height: 104,
		});
		expect(intents[0].intent.action.object.body).toMatchObject({
			serverId: "s1",
			advertisedSourceId: 3,
			outputName: "HDMI 1",
			width: 1920,
		});
		expect(intents[1].intent.action.object.body).toMatchObject({
			widthMetres: 0.6,
			pixelPitchMillimetres: 5.77,
			pixelWidth: 104,
		});
	});

	it("builds surface sections with the kind fields the editor stores in snake_case", async () => {
		const { backend, intents } = memoryBackend();

		await tool("put_media_surface").run(backend, {
			name: "Upstage wall",
			source_id: "src",
			sections: [
				{ type: "led", module_type_id: "m1", rows: 2, columns: 3, width_metres: 1.8, height_metres: 1.2 },
				{
					type: "projection_screen",
					width_metres: 4,
					height_metres: 3,
					position_metres: [0, 0, 2],
					material: { type: "custom", gain: 1.2, tint_srgb: "#ffffff", roughness: 0.4 },
				},
				{ type: "tv", width_metres: 1.2, height_metres: 0.7, bezel_metres: 0.01 },
			],
		});

		const [led, screen, tv] = intents[0].intent.action.object.body.sections;
		expect(intents[0].intent.action.object.body.sourceId).toBe("src");
		expect(led).toMatchObject({
			type: "led",
			module_type_id: "m1",
			rows: 2,
			columns: 3,
			// Every cell holds a module unless holes are named.
			occupied_cells: [0, 1, 2, 3, 4, 5],
			widthMetres: 1.8,
			crop: { left: 0, top: 0, width: 1, height: 1 },
			transform: { positionMetres: [0, 0, 0], rotationDegrees: [0, 0, 0] },
		});
		expect(screen).toMatchObject({
			material: { type: "custom", gain: 1.2, tint_srgb: "#ffffff", roughness: 0.4 },
			edge_feather: 0,
			transform: { positionMetres: [0, 0, 2] },
		});
		expect(tv).toMatchObject({ bezel_metres: 0.01, spill: 0 });
		expect(typeof led.id).toBe("string");

		await expect(
			tool("put_media_surface").run(backend, {
				name: "Bad",
				sections: [{ type: "hologram", width_metres: 1, height_metres: 1 }],
			}),
		).rejects.toThrow("section 1: type must be projection_screen, tv or led");
	});

	it("creates a projector with the Media workspace's defaults and moves it without resetting its optics", async () => {
		const { backend, intents } = memoryBackend();

		const created = (await tool("put_media_projector").run(backend, {
			name: "FOH projector",
			surface_id: "surface",
			position_metres: [0, -15, 6],
		})) as any;
		expect(intents[0].intent.action.object.body).toMatchObject({
			bodyModel: "projector",
			throwRatio: 1.5,
			lensShift: [0, 0],
			coneLengthMetres: 12,
			transform: { positionMetres: [0, -15, 6], rotationDegrees: [0, 0, 0] },
		});

		await tool("put_media_projector").run(backend, { id: created.id, throw_ratio: 2, rotation_degrees: [10, 0, 0] });
		expect(intents[1].intent.action.object.body).toMatchObject({
			throwRatio: 2,
			coneLengthMetres: 12,
			transform: { positionMetres: [0, -15, 6], rotationDegrees: [10, 0, 0] },
		});
	});

	it("deletes an object against its revision and refuses one that is not there", async () => {
		const { backend, intents } = memoryBackend();
		const created = (await tool("put_media_source").run(backend, {
			server_id: "s1",
			advertised_source_id: 1,
			name: "Out",
		})) as any;

		const deleted = (await tool("delete_media_object").run(backend, { kind: "media_source", id: created.id })) as any;

		expect(intents[1].intent).toMatchObject({
			expectedRevision: 1,
			action: { type: "delete", kind: "media_source", id: created.id },
		});
		expect(deleted).toEqual({ kind: "media_source", id: created.id, deleted: true });
		await expect(
			tool("delete_media_object").run(backend, { kind: "media_source", id: created.id }),
		).rejects.toThrow(`no media_source with id ${created.id}`);
		await expect(
			tool("delete_media_object").run(backend, { kind: "media_fallback_asset", id: "x" }),
		).rejects.toThrow("kind must be one of");
	});

	it("says the Control desk has no planned media layout", async () => {
		const { backend } = memoryBackend({ product: "The Control desk", withMedia: false });
		await expect(tool("get_media_layout").run(backend, {})).rejects.toThrow(
			"The Control desk cannot plan a media layout",
		);
		await expect(tool("put_media_server").run(backend, { name: "x" })).rejects.toThrow(
			"The Control desk cannot plan a media layout",
		);
	});
});
