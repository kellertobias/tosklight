import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { CadTileProps } from "./CadApp";
import { CadTileViewport } from "./CadTileViewport";
import type { CadPrintPage, ViewportTile } from "./types";

vi.mock("./cadPreviewStore", () => ({ useLivePreview: () => null }));
vi.mock("./CadViewport", () => ({
	CadViewport: ({ printPages }: { printPages: readonly CadPrintPage[] }) => (
		<ul aria-label="Viewport print frames">
			{printPages.map((page) => <li key={page.id}>{page.name}</li>)}
		</ul>
	),
}));

afterEach(cleanup);

it("keeps every saved rotation's page in its owning tile as the viewport turns", () => {
	const pages: CadPrintPage[] = [0, 1, 2, 3].map((rotation) => ({
		kind: "plan", id: `page-${rotation}`, tileId: "top-tile", name: `Saved rotation ${rotation}`,
		view: "top_down", rotationQuarterTurns: rotation, centreMillimetres: [-1000, 3500],
		widthMillimetres: 6000, included: rotation !== 2, orientation: "landscape",
		showFixtureIds: true, showDmxAddresses: false, showMountingHardware: true,
	}));
	const unrelated: CadPrintPage[] = [
		{ ...pages[0], id: "other-tile", name: "Other tile", tileId: "other-tile" },
		{ ...pages[0], id: "other-view", name: "Other view", view: "front_to_back" },
		{ ...pages[0], id: "fixture-list", name: "Fixture list", kind: "fixture_list" },
	];
	const props = {
		scene: { sceneRevision: 1, entities: [], drawings: [], selectedIds: [] },
		root: { type: "split", id: "split", direction: "row", ratio: 0.5, first: { type: "tile", id: "top-tile", view: "top_down", rotationQuarterTurns: 0, camera: { pan: [0, 0], zoom: 0.1 } }, second: { type: "tile", id: "other-tile", view: "top_down", rotationQuarterTurns: 0, camera: { pan: [0, 0], zoom: 0.1 } } },
		settings: {}, printMode: true, printPages: [...pages, ...unrelated], underlays: [],
		onTile: vi.fn(),
	} as unknown as CadTileProps;
	const node: ViewportTile = {
		type: "tile", id: "top-tile", view: "top_down", rotationQuarterTurns: 0,
		camera: { pan: [0, 0], zoom: 0.1 },
	};
	const { rerender } = render(<CadTileViewport props={props} node={node} />);
	for (const rotation of [0, 1, 2, 3]) {
		rerender(<CadTileViewport props={props} node={{ ...node, rotationQuarterTurns: rotation }} />);
		expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual(pages.map((page) => page.name));
		expect(screen.queryByText("Other tile")).not.toBeInTheDocument();
		expect(screen.queryByText("Other view")).not.toBeInTheDocument();
		expect(screen.queryByText("Fixture list")).not.toBeInTheDocument();
	}
});


it("shows a portable page from another workspace once in the matching current viewport", () => {
	const first = { type: "tile", id: "new-first", view: "top_down", rotationQuarterTurns: 2, camera: { pan: [0, 0], zoom: 0.1 } } as ViewportTile;
	const second = { ...first, id: "new-second" };
	const props = {
		scene: { sceneRevision: 1, entities: [], drawings: [], selectedIds: [] },
		settings: {}, printMode: true, underlays: [], onTile: vi.fn(),
		root: { type: "split", id: "split", direction: "row", ratio: 0.5, first, second },
		printPages: [{ kind: "plan", id: "saved", tileId: "another-computer", name: "Stage Detail", view: "top_down", rotationQuarterTurns: 0 }],
	} as unknown as CadTileProps;
	const { rerender } = render(<CadTileViewport props={props} node={first} />);
	expect(screen.getByText("Stage Detail")).toBeInTheDocument();
	rerender(<CadTileViewport props={props} node={second} />);
	expect(screen.queryByText("Stage Detail")).not.toBeInTheDocument();
});
