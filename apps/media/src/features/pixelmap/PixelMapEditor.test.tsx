import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type {
	OutputConfigurationView,
	PixelMapView,
} from "../../shared/api/generated/media-wire";
import { PixelMapEditor } from "./PixelMapEditor";
import type { PixelMapTab } from "./PixelMapPage";

function output(map?: Partial<PixelMapView>): OutputConfigurationView {
	return {
		id: "output",
		name: "Main",
		width: 1920,
		height: 1080,
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
	} as unknown as OutputConfigurationView;
}

const storedMap: Partial<PixelMapView> = {
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
		{
			id: "zone-floor",
			name: "Floor",
			start: { x: 0.2, y: 0.6 },
			end: { x: 0.8, y: 0.9 },
			columns: 4,
			rows: 2,
			layout: { name: "RGBW", components: ["red", "green", "blue", "white"] },
			order: "serpentine-rows",
			universe: 1,
			startAddress: 31,
			enabled: false,
			footprint: 32,
		},
	] as PixelMapView["zones"],
	regions: [
		{
			id: "region-left",
			name: "Left screen",
			start: { x: 0, y: 0 },
			end: { x: 0.5, y: 1 },
			rotation: "none",
			fit: "fill",
			enabled: true,
		},
		{
			id: "region-right",
			name: "Right screen",
			start: { x: 0.5, y: 0 },
			end: { x: 1, y: 1 },
			rotation: "clockwise-90",
			fit: "contain",
			enabled: true,
		},
	] as PixelMapView["regions"],
};

function Harness({
	configuration,
	onSave,
	initialTab = "regions",
}: {
	configuration: OutputConfigurationView;
	onSave: (map: PixelMapView) => void;
	initialTab?: PixelMapTab;
}) {
	const [tab, setTab] = useState<PixelMapTab>(initialTab);
	return (
		<PixelMapEditor
			output={configuration}
			outputs={[]}
			onOutputChange={vi.fn()}
			tab={tab}
			onTabChange={setTab}
			busy={false}
			failed={false}
			onSave={onSave}
		/>
	);
}

function renderEditor(
	configuration = output(),
	initialTab: PixelMapTab = "regions",
) {
	const onSave = vi.fn();
	render(
		<Harness
			configuration={configuration}
			onSave={onSave}
			initialTab={initialTab}
		/>,
	);
	return onSave;
}

const rowNamed = (name: string) => screen.getByRole("row", { name });
const save = () => screen.getByRole("button", { name: "Save pixel map" });

describe("the Pixel Map dock", () => {
	it("switches between Display Regions and Pixel Zones with the window-title tabs", async () => {
		renderEditor(output(storedMap));
		expect(screen.getByText("Pixel Map")).toHaveClass("ui-window-title");
		const tabs = screen.getByRole("tablist");
		expect(
			within(tabs)
				.getAllByRole("tab")
				.map((tab) => tab.textContent),
		).toEqual(["Display Regions", "Pixel Zones"]);
		expect(
			within(tabs).getByRole("tab", { name: "Display Regions" }),
		).toHaveAttribute("aria-selected", "true");
		expect(
			screen.getByRole("region", { name: "Display regions" }),
		).toBeVisible();
		expect(screen.queryByRole("region", { name: "Pixel zones" })).toBeNull();
		expect(
			screen.getByRole("button", { name: "Add display region" }),
		).toBeVisible();

		await userEvent.click(
			within(tabs).getByRole("tab", { name: "Pixel Zones" }),
		);

		expect(
			within(tabs).getByRole("tab", { name: "Pixel Zones" }),
		).toHaveAttribute("aria-selected", "true");
		expect(screen.getByRole("region", { name: "Pixel zones" })).toBeVisible();
		expect(
			screen.getByRole("region", { name: "Media Server DMX output" }),
		).toBeVisible();
		expect(
			screen.queryByRole("region", { name: "Display regions" }),
		).toBeNull();
		expect(
			screen.getByRole("button", { name: "Add pixel zone" }),
		).toBeVisible();
		// Only the open tab's shapes answer a press on the picture.
		expect(
			screen.getByRole("button", { name: "Truss pixel zone" }),
		).toBeEnabled();
		expect(
			screen.getByRole("button", { name: "Left screen display region" }),
		).toBeDisabled();
	});

	it("keeps the add action as wide as the longest label either tab needs", async () => {
		renderEditor(output(storedMap));
		const tabs = screen.getByRole("tablist");
		const labels = (button: HTMLElement) =>
			Array.from(button.querySelectorAll(".media-pixel-map-add-label > span")).map(
				(span) => [span.textContent, span.classList.contains("is-sizer")],
			);

		// Both labels are laid out in the same grid cell, so the button is sized by the longer one
		// and switching tabs cannot move the title bar around it.
		expect(labels(screen.getByRole("button", { name: "Add display region" }))).toEqual(
			[
				["Add display region", false],
				["Add pixel zone", true],
			],
		);

		await userEvent.click(
			within(tabs).getByRole("tab", { name: "Pixel Zones" }),
		);

		expect(labels(screen.getByRole("button", { name: "Add pixel zone" }))).toEqual([
			["Add display region", true],
			["Add pixel zone", false],
		]);
	});

	it("shows the output picture beside the configuration", () => {
		renderEditor(output(storedMap));
		const picture = screen.getByRole("group", {
			name: "Output picture, 1920 by 1080",
		});
		const frame = picture.querySelector("img");
		expect(frame).toHaveAttribute(
			"src",
			expect.stringContaining("/outputs/output/preview"),
		);
		expect(
			within(picture).getByRole("button", {
				name: "Left screen display region",
			}),
		).toBeVisible();
		expect(
			within(picture).getByRole("button", { name: "Truss pixel zone" }),
		).toBeInTheDocument();
	});

	it("draws a smaller region above a larger one so both can be pressed", async () => {
		const [left, right] = storedMap.regions as PixelMapView["regions"];
		renderEditor(
			output({
				regions: [
					{
						...left,
						id: "wall",
						name: "Wall",
						end: { x: 1, y: 1 },
					},
					{
						...right,
						id: "inset",
						name: "Inset",
						start: { x: 0.4, y: 0.4 },
						end: { x: 0.6, y: 0.6 },
					},
				] as PixelMapView["regions"],
			}),
		);
		const shapes = Array.from(
			document.querySelectorAll(".media-pixel-region"),
		).map((shape) => shape.getAttribute("aria-label"));
		expect(shapes).toEqual(["Wall display region", "Inset display region"]);
		// Selecting the large one does not lift it over the inset.
		await userEvent.click(
			screen.getByRole("button", { name: "Inset display region" }),
		);
		expect(rowNamed("Inset")).toHaveAttribute("aria-selected", "true");
	});

	it("opens a stored map in its tables without losing anything", async () => {
		const onSave = renderEditor(output(storedMap));
		expect(screen.getByLabelText("Left screen name")).toHaveValue(
			"Left screen",
		);
		expect(screen.getByLabelText("Right screen right")).toHaveValue("1");
		expect(
			screen.getByRole("button", { name: "Right screen rotation" }),
		).toHaveTextContent("Turned clockwise");
		expect(
			screen.getByRole("button", { name: "Right screen fit" }),
		).toHaveTextContent("Fit on the screen");
		// Nothing changed, so there is nothing to save.
		expect(save()).toBeDisabled();

		await userEvent.click(screen.getByRole("tab", { name: "Pixel Zones" }));
		expect(screen.getByLabelText("Floor pixels across")).toHaveValue("4");
		expect(screen.getByLabelText("Floor output address")).toHaveValue("31");
		expect(
			screen.getByRole("button", { name: "Floor wiring order" }),
		).toHaveTextContent("Rows, folded");
		expect(
			screen.getByRole("switch", { name: "Send Floor" }),
		).not.toBeChecked();
		expect(screen.getByLabelText("Universe 1 route name")).toHaveValue(
			"Universe 1",
		);

		// A single edit saves the whole stored map, with every other value untouched.
		await replace("Floor output address", "40");
		await userEvent.click(save());
		const saved = onSave.mock.calls[0][0] as PixelMapView;
		expect(saved.regions).toEqual(storedMap.regions);
		expect(saved.zones[0]).toEqual(storedMap.zones?.[0]);
		expect(saved.zones[1]).toEqual({
			...storedMap.zones?.[1],
			startAddress: 40,
		});
	});

	it("edits a zone in its table row and recounts its footprint", async () => {
		const onSave = renderEditor(output(), "zones");
		expect(screen.getByText(/No pixel zone yet/)).toBeVisible();
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);

		expect(rowNamed("Zone 1")).toHaveAttribute("aria-selected", "true");
		expect(screen.getByLabelText("Zone 1 pixels across")).toHaveValue("12");
		await replace("Zone 1 pixels across", "24");
		expect(within(rowNamed("Zone 1 patch")).getByText("72")).toBeVisible();
		await replace("Zone 1 name", "Rail");
		await userEvent.click(screen.getByRole("switch", { name: "Send Rail" }));
		await userEvent.click(save());

		const saved = onSave.mock.calls[0][0] as PixelMapView;
		expect(saved.zones).toHaveLength(1);
		expect(saved.zones[0]).toMatchObject({
			name: "Rail",
			columns: 24,
			enabled: false,
			// Twenty-four RGB pixels is seventy-two slots, recounted rather than left stale.
			footprint: 72,
		});
	});

	it("edits display regions and output routes in their tables", async () => {
		const onSave = renderEditor(output());
		await userEvent.click(
			screen.getByRole("button", { name: "Add display region" }),
		);
		await replace("Screen 1 right", "0.5");
		await choose("Screen 1 rotation", "Upside down");

		await userEvent.click(screen.getByRole("tab", { name: "Pixel Zones" }));
		await userEvent.click(
			screen.getByRole("button", { name: "Add output route" }),
		);
		await replace("Universe 2 destination", "10.0.0.9");
		await userEvent.click(save());

		const saved = onSave.mock.calls[0][0] as PixelMapView;
		expect(saved.regions).toHaveLength(1);
		expect(saved.regions[0]).toMatchObject({
			end: { x: 0.5, y: 1 },
			rotation: "half",
		});
		expect(saved.routes).toHaveLength(2);
		expect(saved.routes[1].destination).toBe("10.0.0.9");
	});

	it("keeps the selected row and its shape on the picture in step", async () => {
		renderEditor(output(storedMap));
		const left = screen.getByRole("button", {
			name: "Left screen display region",
		});
		const right = screen.getByRole("button", {
			name: "Right screen display region",
		});
		// The first stored region starts selected, in the table and on the picture.
		expect(rowNamed("Left screen")).toHaveAttribute("aria-selected", "true");
		expect(left).toHaveAttribute("aria-pressed", "true");

		// Picture to table.
		await userEvent.click(right);
		expect(right).toHaveAttribute("aria-pressed", "true");
		expect(left).toHaveAttribute("aria-pressed", "false");
		expect(rowNamed("Right screen")).toHaveAttribute("aria-selected", "true");
		expect(rowNamed("Left screen")).toHaveAttribute("aria-selected", "false");

		// Table to picture, including by editing a cell of another row.
		await userEvent.click(screen.getByLabelText("Left screen name"));
		expect(rowNamed("Left screen")).toHaveAttribute("aria-selected", "true");
		expect(left).toHaveAttribute("aria-pressed", "true");

		await userEvent.click(screen.getByRole("tab", { name: "Pixel Zones" }));
		const floor = screen.getByRole("button", { name: "Floor pixel zone" });
		expect(rowNamed("Truss")).toHaveAttribute("aria-selected", "true");
		await userEvent.click(floor);
		expect(floor).toHaveAttribute("aria-pressed", "true");
		expect(rowNamed("Floor")).toHaveAttribute("aria-selected", "true");
		await userEvent.click(rowNamed("Truss"));
		expect(
			screen.getByRole("button", { name: "Truss pixel zone" }),
		).toHaveAttribute("aria-pressed", "true");
		expect(floor).toHaveAttribute("aria-pressed", "false");
	});

	it("moves and resizes the selected shape by dragging it on the picture", async () => {
		const onSave = renderEditor(output(storedMap));
		const picture = screen.getByRole("group", {
			name: "Output picture, 1920 by 1080",
		});
		// jsdom lays nothing out, so the picture is given a size to measure drags against.
		vi.spyOn(picture, "getBoundingClientRect").mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			width: 400,
			height: 200,
			right: 400,
			bottom: 200,
			toJSON: () => ({}),
		});
		const right = screen.getByRole("button", {
			name: "Right screen display region",
		});
		const drag = (target: Element, dx: number, dy: number) => {
			fireEvent.pointerDown(target, {
				pointerId: 1,
				button: 0,
				clientX: 100,
				clientY: 100,
			});
			fireEvent.pointerMove(target, {
				pointerId: 1,
				clientX: 100 + dx,
				clientY: 100 + dy,
			});
			fireEvent.pointerUp(target, { pointerId: 1 });
		};

		// A press that barely moves is a tap: it selects and changes nothing.
		drag(right, 2, 1);
		fireEvent.click(right);
		expect(right).toHaveAttribute("aria-pressed", "true");
		expect(rowNamed("Right screen")).toHaveAttribute("aria-selected", "true");
		expect(save()).toBeDisabled();

		// Dragging the body moves the region; its right edge stops at the canvas edge.
		drag(right, -40, 20);
		expect(screen.getByLabelText("Right screen left")).toHaveValue("0.4");
		expect(screen.getByLabelText("Right screen right")).toHaveValue("0.9");
		expect(screen.getByLabelText("Right screen top")).toHaveValue("0");

		// The selected region's corner handles resize it.
		const handle = picture.querySelector(
			'.media-pixel-handle[data-handle="bottom-right"]',
		);
		if (!handle) throw new Error("The selected region has no handles");
		drag(handle, -80, -100);
		expect(screen.getByLabelText("Right screen right")).toHaveValue("0.7");
		expect(screen.getByLabelText("Right screen bottom")).toHaveValue("0.5");

		// The arrow keys nudge the selected region too.
		right.focus();
		await userEvent.keyboard("{ArrowRight}");
		expect(screen.getByLabelText("Right screen left")).toHaveValue("0.41");

		await userEvent.click(save());
		const saved = onSave.mock.calls[0][0] as PixelMapView;
		expect(saved.regions[1]).toEqual({
			...storedMap.regions?.[1],
			start: { x: 0.41, y: 0 },
			end: { x: 0.71, y: 0.5 },
		});
		expect(saved.regions[0]).toEqual(storedMap.regions?.[0]);
	});

	it("removes a zone from its row", async () => {
		renderEditor(output(), "zones");
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Remove Zone 1" }),
		);
		expect(screen.getByText(/No pixel zone yet/)).toBeVisible();
		expect(
			screen.queryByRole("button", { name: "Zone 1 pixel zone" }),
		).toBeNull();
	});

	it("refuses to save a map with a problem, and says what it is", async () => {
		const onSave = renderEditor(output(), "zones");
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);
		await replace("Zone 1 output universe", "9");

		expect(screen.getByLabelText("Pixel map problems")).toHaveTextContent(
			"no enabled output route carries",
		);
		expect(save()).toBeDisabled();
		expect(onSave).not.toHaveBeenCalled();
	});

	it("creates an explicit desk input handoff when desk merge is selected", async () => {
		const onSave = renderEditor(output(), "zones");
		await userEvent.click(
			screen.getByRole("button", { name: "Add pixel zone" }),
		);
		await userEvent.click(
			screen.getByRole("button", { name: "Direct Media Server output" }),
		);
		await userEvent.click(screen.getByRole("option", { name: "Desk merge" }));
		const handoff = screen.getByRole("group", { name: "Zone 1 desk handoff" });
		expect(within(handoff).getByLabelText("Desk input universe")).toHaveValue(
			"1",
		);
		expect(within(handoff).getByLabelText("Dimmer address")).toHaveValue("1");
		expect(within(handoff).getByLabelText("Mix address")).toHaveValue("2");
		await userEvent.click(save());
		const saved = onSave.mock.calls[0][0] as PixelMapView;
		expect(saved.mode).toBe("desk-merge");
		expect(saved.handoffs).toHaveLength(1);
	});
});

async function choose(label: string, option: string) {
	await userEvent.click(screen.getByRole("button", { name: label }));
	await userEvent.click(screen.getByRole("option", { name: option }));
}

async function replace(label: string, value: string) {
	const field = screen.getByLabelText(label);
	await userEvent.clear(field);
	await userEvent.type(field, value);
}
