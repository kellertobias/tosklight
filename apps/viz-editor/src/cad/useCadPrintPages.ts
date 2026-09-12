/**
 * The print pages of the CAD screen: the pages themselves, which one is selected, and the
 * workspace-local storage they survive a restart in.
 *
 * Pages are workspace state rather than show data — they belong to the operator composing
 * paperwork on this machine, not to the rig — so they live in localStorage beside the tile layout.
 */
import { useEffect, useState } from "react";
import { rotatePrintPage } from "./print";
import {
	type CadPrintPage,
	legacyTopDownPlanPoint,
	type ViewportTile,
} from "./types";

const PRINT_KEY = "tosklight:viz-editor:cad-print-pages:v2";
const LEGACY_PRINT_KEY = "tosklight:viz-editor:cad-print-pages:v1";

function pageId(prefix: string, pageNumber: number): string {
	return (
		globalThis.crypto?.randomUUID?.() ?? `${prefix}-${Date.now()}-${pageNumber}`
	);
}

/**
 * The pages a previous session left behind.
 *
 * A page is kept only when it still carries the fields a sheet needs; everything optional is
 * normalised here so the rest of the screen never has to ask what an older file meant. v1 stored
 * top-down plans mirrored, and is converted once on first read.
 */
export function restorePrintPages(): CadPrintPage[] {
	try {
		const current = localStorage.getItem(PRINT_KEY);
		const legacy = current == null;
		const stored = JSON.parse(
			current ?? localStorage.getItem(LEGACY_PRINT_KEY) ?? "[]",
		);
		if (!Array.isArray(stored)) return [];
		return stored
			.filter(
				(page): page is CadPrintPage =>
					typeof page?.id === "string" &&
					typeof page?.tileId === "string" &&
					typeof page?.name === "string" &&
					page?.centreMillimetres?.length === 2 &&
					Number.isFinite(page?.widthMillimetres),
			)
			.map((page) => ({
				...page,
				centreMillimetres:
					legacy && page.view === "top_down"
						? legacyTopDownPlanPoint(
								page.centreMillimetres,
								page.rotationQuarterTurns ?? 0,
							)
						: page.centreMillimetres,
				kind: page.kind === "fixture_list" ? "fixture_list" : "plan",
				orientation: page.orientation === "portrait" ? "portrait" : "landscape",
				showFixtureIds: page.showFixtureIds === true,
				showDmxAddresses: page.showDmxAddresses === true,
			}));
	} catch {
		return [];
	}
}

export interface CadPrintPages {
	pages: CadPrintPage[];
	selectedId: string | null;
	select(id: string | null): void;
	/** A plan page framing what a tile currently shows, including its depth slice. */
	addPlanPage(tile: ViewportTile): void;
	addFixtureList(): void;
	change(id: string, change: Partial<CadPrintPage>): void;
	rotate(id: string): void;
}

export function useCadPrintPages(): CadPrintPages {
	const [pages, setPages] = useState<CadPrintPage[]>(restorePrintPages);
	const [selectedId, setSelectedId] = useState<string | null>(null);

	useEffect(() => {
		localStorage.setItem(PRINT_KEY, JSON.stringify(pages));
	}, [pages]);

	function add(page: CadPrintPage) {
		setPages((current) => [...current, page]);
		setSelectedId(page.id);
	}

	return {
		pages,
		selectedId,
		select: setSelectedId,
		addPlanPage(tile) {
			const pageNumber = pages.length + 1;
			add({
				kind: "plan",
				id: pageId("page", pageNumber),
				tileId: tile.id,
				name: `Page ${pageNumber}`,
				view: tile.view,
				rotationQuarterTurns: tile.rotationQuarterTurns,
				cutPlanes: tile.cutPlanes,
				centreMillimetres: [-tile.camera.pan[0], -tile.camera.pan[1]],
				widthMillimetres: Math.max(3000, 360 / tile.camera.zoom),
				included: true,
				orientation: "landscape",
				showFixtureIds: false,
				showDmxAddresses: false,
			});
		},
		addFixtureList() {
			const pageNumber = pages.length + 1;
			add({
				kind: "fixture_list",
				id: pageId("fixture-list", pageNumber),
				tileId: "fixture-list",
				name: "Fixture List",
				view: "top_down",
				rotationQuarterTurns: 0,
				centreMillimetres: [0, 0],
				widthMillimetres: 2970,
				included: true,
				orientation: "landscape",
				showFixtureIds: true,
				showDmxAddresses: true,
			});
		},
		change(id, change) {
			setPages((current) =>
				current.map((page) => (page.id === id ? { ...page, ...change } : page)),
			);
		},
		rotate(id) {
			setPages((current) =>
				current.map((page) => (page.id === id ? rotatePrintPage(page) : page)),
			);
		},
	};
}
