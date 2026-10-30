import fs from "node:fs";
import { describe, expect, it } from "vitest";
import { installPlannedDemoOutputRoutes } from "../../support/plannedDemoGenerator";
import { installPlannedDemoMedia } from "../../support/plannedDemoMedia";

describe("default show Pixel companion", () => {
	it("maps real Program identities and distinct wall crops to the portable configuration", async () => {
		const configuration = JSON.parse(
			fs.readFileSync(
				new URL(
					"../../../assets/demo-media/media-server.json",
					import.meta.url,
				),
				"utf8",
			),
		).configuration;
		const writes: Array<{ kind: string; id: string; body: any }> = [];
		const api = {
			showObjects: async () => [],
			showObject: async () => null,
			seedShowObject: async (
				_show: string,
				kind: string,
				id: string,
				body: any,
			) => {
				writes.push({ kind, id, body });
			},
		} as any;
		const media = await installPlannedDemoMedia(api, "show");
		const routes = await installPlannedDemoOutputRoutes(api, "show");
		const lighting = routes.filter((route) => route.body.protocol === "sacn");
		expect(lighting.map((route) => route.body.logical_universe)).toEqual([
			1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
		]);
		expect(
			lighting.every(
				(route) => route.body.destination !== configuration.network.sacnListen,
			),
		).toBe(true);
		for (const output of configuration.outputs)
			expect(
				routes.some(
					(route) =>
						route.body.protocol === "art_net" &&
						route.body.logical_universe === 9 &&
						route.body.destination_universe === output.universe,
				),
			).toBe(true);

		expect(media.servers.every((server) => server.citp.port === 4809)).toBe(
			true,
		);
		const programId = (id: string) => {
			const bytes = Buffer.from(id.replaceAll("-", ""), "hex");
			return bytes.readUInt16LE(0);
		};
		// Pixel also reserves the two layer identities after each Program identity.
		const reserved = configuration.outputs.flatMap((output: any) =>
			[0, 1, 2].map((offset) => programId(output.id) + offset),
		);
		expect(new Set(reserved).size).toBe(9);
		expect(
			media.sources
				.map((source) => source.advertisedSourceId)
				.sort((a, b) => a - b),
		).toEqual([257, 513, 769]);
		for (const source of media.sources) {
			const output = configuration.outputs.find(
				(output: any) => programId(output.id) === source.advertisedSourceId,
			);
			expect(output).toBeDefined();
			expect(output.resolution).toEqual({
				width: source.width,
				height: source.height,
			});
			expect(output.personality).toBe("two-layers");
			expect(output.universe).toBe(source.advertisedSourceId === 769 ? 10 : 9);
			expect([177, 335]).toContain(output.startAddress);
		}
		const walls = media.surfaces.find(
			(surface) => surface.name === "Sunstrip LED Panels",
		)!.sections;
		expect(walls.map((section) => section.transform.positionMetres[0])).toEqual(
			[-3, 0, 3],
		);
		expect(walls.map((section) => section.crop)).toEqual(
			[0, 1, 2].map((segment) => ({
				left: segment / 3,
				top: 0,
				width: 1 / 3,
				height: 1,
			})),
		);
		expect(media.surfaces.every((surface) => surface.fallback === null)).toBe(
			true,
		);
	});
});
