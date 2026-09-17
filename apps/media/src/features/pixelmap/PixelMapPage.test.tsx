import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../../app/ToastContext";
import type {
	OutputConfigurationView,
	PixelMapView,
} from "../../shared/api/generated/media-wire";
import {
	anOutput,
	anOutputConfiguration,
	stubServer,
} from "../../testing/server";
import { PixelMapPage } from "./PixelMapPage";

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

const OUTPUT_ID = anOutput().id;

const stored: PixelMapView = {
	mode: "direct",
	zones: [
		{
			id: "zone-truss",
			name: "Truss",
			start: { x: 0.1, y: 0.1 },
			end: { x: 0.9, y: 0.2 },
			columns: 10,
			rows: 1,
			layout: { name: "RGB", components: ["red", "green", "blue"] },
			order: "row-major",
			universe: 1,
			startAddress: 1,
			enabled: true,
			footprint: 30,
		},
	],
	routes: [
		{
			id: "route",
			name: "Universe 1",
			protocol: "art-net",
			universe: 1,
			destination: null,
			enabled: true,
		},
	],
	handoffs: [],
	regions: [
		{
			id: "region-wall",
			name: "Wall",
			start: { x: 0, y: 0 },
			end: { x: 1, y: 1 },
			rotation: "half",
			fit: "contain",
			enabled: true,
		},
	],
} as unknown as PixelMapView;

function installStoredMap() {
	stubServer();
	const configuration = anOutputConfiguration(OUTPUT_ID, "Main", {
		pixelMap: structuredClone(stored),
	}) as OutputConfigurationView;
	const writes: Array<Record<string, unknown>> = [];
	const baseFetch = globalThis.fetch;
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const path = String(input).replace("/api/v2", "");
			if (path === `/outputs/${OUTPUT_ID}/configuration`)
				return Response.json(configuration);
			if (path === `/outputs/${OUTPUT_ID}/configuration/update`) {
				const body = JSON.parse(String(init?.body ?? "{}"));
				writes.push(body);
				Object.assign(configuration, { pixelMap: body.pixelMap });
				return Response.json(configuration);
			}
			return baseFetch(input, init);
		}),
	);
	return { configuration, writes };
}

describe("the Pixel Map page", () => {
	it("opens the stored pixel map in its own window and saves only the map", async () => {
		const { writes } = installStoredMap();
		render(
			<ToastProvider>
				<PixelMapPage />
			</ToastProvider>,
		);

		expect(await screen.findByLabelText("Wall name")).toHaveValue("Wall");
		expect(
			screen.getByRole("button", { name: "Wall rotation" }),
		).toHaveTextContent("Upside down");
		await userEvent.click(screen.getByRole("tab", { name: "Pixel Zones" }));
		expect(screen.getByLabelText("Truss pixels across")).toHaveValue("10");

		const across = screen.getByLabelText("Truss pixels across");
		await userEvent.clear(across);
		await userEvent.type(across, "20");
		await userEvent.click(
			screen.getByRole("button", { name: "Save pixel map" }),
		);

		await waitFor(() => expect(writes).toHaveLength(1));
		expect(Object.keys(writes[0]).sort()).toEqual(["pixelMap", "requestId"]);
		const saved = writes[0].pixelMap as PixelMapView;
		expect(saved.regions).toEqual(stored.regions);
		expect(saved.routes).toEqual(stored.routes);
		expect(saved.zones[0]).toEqual({
			...stored.zones[0],
			columns: 20,
			footprint: 60,
		});
		// The page re-reads the stored map, and keeps the tab the operator was on.
		await waitFor(() =>
			expect(screen.getByLabelText("Truss pixels across")).toHaveValue("20"),
		);
		expect(screen.getByRole("tab", { name: "Pixel Zones" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
	});
});
