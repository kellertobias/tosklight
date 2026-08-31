import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
	OutputConfigurationView,
	PixelMapView,
} from "../../shared/api/generated/media-wire";
import { PixelMapSettings } from "./PixelMapSettings";

function output(map?: Partial<PixelMapView>): OutputConfigurationView {
	return {
		id: "output",
		name: "Main",
		targetKind: "monitor",
		monitorBy: "index",
		monitorValue: "0",
		fullscreen: true,
		width: 1920,
		height: 1080,
		presentation: "display-synchronized",
		framesPerSecond: null,
		soundOutputKind: "disabled",
		soundOutputName: null,
		availableMonitors: [],
		availableSoundOutputs: [],
		pixelMap: {
			mode: "direct",
			zones: [],
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
			regions: [],
			...map,
		},
		personality: "two-layers",
		personalityLayout: "extended",
		protocol: "art-net",
		universe: 1,
		startAddress: 1,
		active: undefined as never,
		takesEffectOnRestart: false,
	} as unknown as OutputConfigurationView;
}

describe("the pixel map editor", () => {
	it("adds a zone, shows it on the canvas, and offers it for editing", async () => {
		render(
			<PixelMapSettings output={output()} busy={false} onSave={vi.fn()} />,
		);
		expect(screen.getByText(/No pixel zone selected/)).toBeVisible();

		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);

		// It appears on the canvas and its fields open for editing.
		expect(screen.getByRole("button", { name: /Zone 1/ })).toBeVisible();
		expect(screen.getByLabelText("Pixels across")).toHaveValue("12");
	});

	it("saves the map it was given back, with the edits made to it", async () => {
		const onSave = vi.fn();
		render(<PixelMapSettings output={output()} busy={false} onSave={onSave} />);
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);
		await replaceNumber("Pixels across", "24");
		await userEvent.click(
			screen.getByRole("button", { name: "Save pixel map" }),
		);

		expect(onSave).toHaveBeenCalledTimes(1);
		const saved = onSave.mock.calls[0][0] as PixelMapView;
		expect(saved.zones).toHaveLength(1);
		expect(saved.zones[0].columns).toBe(24);
		// Twenty-four RGB pixels is seventy-two slots, recounted rather than left stale.
		expect(saved.zones[0].footprint).toBe(72);
	});

	it("removes the zone it is showing", async () => {
		render(
			<PixelMapSettings output={output()} busy={false} onSave={vi.fn()} />,
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);
		await userEvent.click(screen.getByRole("button", { name: "Remove zone" }));
		expect(screen.getByText(/No pixel zone selected/)).toBeVisible();
	});

	it("refuses to save a map with a problem, and says what it is", async () => {
		const onSave = vi.fn();
		render(<PixelMapSettings output={output()} busy={false} onSave={onSave} />);
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);
		// The zone's own universe, not the route's.
		const zoneEditor = screen.getByRole("button", { name: "Remove zone" })
			.parentElement as HTMLElement;
		await replaceNumber("Media Server output universe", "9", zoneEditor);

		const problems = screen.getByLabelText("Pixel map problems");
		expect(problems).toHaveTextContent("no enabled output route carries");
		expect(
			screen.getByRole("button", { name: "Save pixel map" }),
		).toBeDisabled();
	});

	it("adds a display region and an output route", async () => {
		const onSave = vi.fn();
		render(<PixelMapSettings output={output()} busy={false} onSave={onSave} />);
		await userEvent.click(
			screen.getByRole("button", { name: "Add display region" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Add output route" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Save pixel map" }),
		);

		const saved = onSave.mock.calls[0][0] as PixelMapView;
		expect(saved.regions).toHaveLength(1);
		expect(saved.routes).toHaveLength(2);
	});

	it("creates an explicit desk input handoff when desk merge is selected", async () => {
		const onSave = vi.fn();
		render(<PixelMapSettings output={output()} busy={false} onSave={onSave} />);
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);
		await choose("Direct Media Server output", "Desk merge");
		expect(screen.getByLabelText("Desk input universe")).toHaveValue("1");
		expect(screen.getByLabelText("Dimmer address")).toHaveValue("1");
		expect(screen.getByLabelText("Mix address")).toHaveValue("2");
		await userEvent.click(
			screen.getByRole("button", { name: "Save pixel map" }),
		);
		expect((onSave.mock.calls[0][0] as PixelMapView).mode).toBe("desk-merge");
		expect((onSave.mock.calls[0][0] as PixelMapView).handoffs).toHaveLength(1);
	});
});

async function choose(current: string, next: string) {
	await userEvent.click(screen.getByRole("button", { name: current }));
	await userEvent.click(screen.getByRole("option", { name: next }));
}

async function replaceNumber(
	label: string,
	value: string,
	scope?: HTMLElement,
) {
	const field = (scope ? within(scope) : screen).getByLabelText(label);
	await userEvent.clear(field);
	await userEvent.type(field, value);
}
