/**
 * The CAD screen's side panel: **Plans**, **Elements** and **Info**.
 *
 * One panel at a time opens from the title; its tabs and its **+** button share the panel's own title
 * row. Info follows the selection: it sits at the foot of an open panel, and when no panel is open it
 * is the whole side panel. The panel is as wide as the operator last dragged it.
 */
import {
	Button,
	TitleChrome,
	type TitleActionGroup,
	type TitleDropdownItem,
} from "@tosklight/ui";
import { type KeyboardEvent, type PointerEvent, useRef, useState } from "react";
import { CadElementsPanel, type ElementsRequests, type ElementsTab, elementsAddItems } from "./CadElementsPanel";
import { CadInfoPanel } from "./CadInfoPanel";
import type { CadTools } from "./cadTools";
import { CAD_VIEW_LABELS, type CadEntity, type CadSceneSnapshot, type CadViewDirection } from "./types";
import type { CadUnderlays } from "./useCadUnderlays";
import type { useCadPrintPages } from "./useCadPrintPages";
import "./cadSidebar.css";

export type CadPanel = "print" | "elements" | null;

const WIDTH_KEY = "tosklight:viz-editor:cad-sidebar-width:v1";
const MIN_WIDTH = 240;
const MAX_WIDTH = 640;
const DEFAULT_WIDTH = 300;

function clampWidth(width: number) {
	return Math.round(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width)));
}

function useStoredWidth() {
	const [width, setWidth] = useState(() => {
		try {
			const stored = Number(localStorage.getItem(WIDTH_KEY));
			return stored ? clampWidth(stored) : DEFAULT_WIDTH;
		} catch {
			return DEFAULT_WIDTH;
		}
	});
	const change = (next: number) => {
		const clamped = clampWidth(next);
		setWidth(clamped);
		try {
			localStorage.setItem(WIDTH_KEY, String(clamped));
		} catch {
			// A panel that cannot remember its width still resizes.
		}
	};
	return [width, change] as const;
}

/** The left edge of the panel, dragged with the pointer or moved with the arrow keys. */
function ResizeHandle({ width, onWidth }: { width: number; onWidth(width: number): void }) {
	const drag = useRef<{ startX: number; startWidth: number } | null>(null);
	return (
		<div
			className="cad-sidebar-resize"
			role="separator"
			aria-orientation="vertical"
			aria-label="Resize side panel"
			aria-valuemin={MIN_WIDTH}
			aria-valuemax={MAX_WIDTH}
			aria-valuenow={width}
			tabIndex={0}
			onPointerDown={(event: PointerEvent<HTMLDivElement>) => {
				if (event.button !== 0) return;
				event.currentTarget.setPointerCapture?.(event.pointerId);
				drag.current = { startX: event.clientX, startWidth: width };
			}}
			onPointerMove={(event) => {
				if (!drag.current) return;
				// The handle is on the panel's left edge, so dragging left widens it.
				onWidth(drag.current.startWidth + drag.current.startX - event.clientX);
			}}
			onPointerUp={(event) => {
				drag.current = null;
				event.currentTarget.releasePointerCapture?.(event.pointerId);
			}}
			onKeyDown={(event: KeyboardEvent<HTMLDivElement>) => {
				if (event.key === "ArrowLeft") onWidth(width + 20);
				if (event.key === "ArrowRight") onWidth(width - 20);
			}}
		/>
	);
}

type PrintPages = ReturnType<typeof useCadPrintPages>;

function PlansPanel({
	printPages,
	exporting,
	onExport,
}: {
	printPages: PrintPages;
	exporting: boolean;
	onExport(): void;
}) {
	const { pages, selectedId } = printPages;
	const selected = pages.find((page) => page.id === selectedId);
	return (
		<>
			<div className="cad-print-list">
				{pages.length ? (
					pages.map((page, index) => (
						<div
							key={page.id}
							className={`cad-print-row ${page.id === selectedId ? "is-selected" : ""}`}
						>
							<input
								aria-label={`Include ${page.name}`}
								type="checkbox"
								checked={page.included}
								onChange={(event) =>
									printPages.change(page.id, { included: event.currentTarget.checked })
								}
							/>
							<button type="button" onClick={() => printPages.select(page.id)}>
								<strong>
									{index + 1}. {page.name}
								</strong>
								<small>
									{page.kind === "fixture_list" ? "Fixture table" : CAD_VIEW_LABELS[page.view]}
								</small>
								<small>A4 {page.orientation}</small>
							</button>
						</div>
					))
				) : (
					<p>Add a page from any view.</p>
				)}
			</div>
			{selected && selected.kind !== "fixture_list" ? (
				<section className="cad-print-page-settings" aria-label="Selected page settings">
					<Button onClick={() => printPages.rotate(selected.id)}>Rotate page</Button>
				</section>
			) : null}
			<Button
				className="cad-export-pdf"
				disabled={exporting || !pages.some((page) => page.included)}
				onClick={onExport}
			>
				{exporting ? "Exporting…" : "Export to PDF"}
			</Button>
		</>
	);
}

/** The element the selection names, when it names exactly one. */
function selectedEntity(scene: CadSceneSnapshot | null): CadEntity | null {
	if (!scene || scene.selectedIds.length !== 1) return null;
	const id = scene.selectedIds[0];
	return (
		scene.entities.find((entity) => entity.logicalFixtureId === id) ??
		scene.entities.find((entity) => entity.id === id) ??
		null
	);
}

const addGroup = (label: string, items: TitleDropdownItem[]): TitleActionGroup => ({
	id: "panel-add",
	actions: [
		{
			id: "add",
			kind: "dropdown",
			icon: <span aria-hidden="true">+</span>,
			ariaLabel: label,
			dropdown: { kind: "items", ariaLabel: label, items },
		},
	],
});

export function CadSidePanels({
	panel,
	scene,
	tools,
	underlayState,
	defaultView,
	documentKey,
	printPages,
	exporting,
	onExport,
	onSelect,
	onError,
}: {
	panel: CadPanel;
	scene: CadSceneSnapshot | null;
	tools: CadTools;
	underlayState: CadUnderlays;
	defaultView: CadViewDirection;
	documentKey: string | null;
	printPages: PrintPages;
	exporting: boolean;
	onExport(): void;
	onSelect(ids: string[]): void;
	onError(reason: unknown): void;
}) {
	const [width, setWidth] = useStoredWidth();
	const [tab, setTab] = useState<ElementsTab>("drawings");
	const [requests, setRequests] = useState<ElementsRequests>({
		newFolder: 0,
		chooseDrawing: 0,
		importModel: 0,
	});
	const selectionCount = scene?.selectedIds.length ?? 0;
	if (!panel && selectionCount === 0) return null;

	const request = (kind: keyof ElementsRequests) =>
		setRequests((current) => ({ ...current, [kind]: current[kind] + 1 }));
	const title = panel === "print" ? "Plans" : panel === "elements" ? "Elements" : "Info";
	const groups: TitleActionGroup[] =
		panel === "elements"
			? [
					{
						id: "elements-tabs",
						kind: "tabs",
						activeId: tab,
						onActiveChange: (id) => setTab(id as ElementsTab),
						actions: [
							{ id: "drawings", label: "Drawings" },
							{ id: "objects", label: "Objects" },
						],
					},
					addGroup(
						tab === "drawings" ? "Add drawing" : "Add object",
						elementsAddItems(tab, tools, request),
					),
				]
			: panel === "print"
				? [
						addGroup("Add plan page", [
							{
								kind: "action",
								id: "fixture-list",
								label: "Fixture list",
								onPress: printPages.addFixtureList,
							},
						]),
					]
				: [];

	return (
		<aside
			className={`cad-print-sidebar cad-sidebar ${panel ? "" : "is-info-only"}`.trim()}
			aria-label={title}
			style={{ width: `${width}px` }}
		>
			<ResizeHandle width={width} onWidth={setWidth} />
			<header className="cad-sidebar-header">
				<h2>{title}</h2>
				{groups.length ? (
					<TitleChrome
						className="ui-window-action-groups"
						groupClassName="ui-window-action-group"
						terminalActions={[]}
						groups={groups}
					/>
				) : null}
			</header>
			{panel ? (
				<div className="cad-sidebar-body">
					{panel === "elements" && scene ? (
						<CadElementsPanel
							tab={tab}
							requests={requests}
							documentKey={documentKey}
							underlayState={underlayState}
							tools={tools}
							defaultView={defaultView}
							entities={scene.entities}
							selectedIds={scene.selectedIds}
							onSelect={onSelect}
						/>
					) : null}
					{panel === "print" ? (
						<PlansPanel printPages={printPages} exporting={exporting} onExport={onExport} />
					) : null}
				</div>
			) : null}
			{selectionCount > 0 && scene ? (
				<div className="cad-sidebar-info">
					<CadInfoPanel
						entity={selectedEntity(scene)}
						selectionCount={selectionCount}
						sceneRevision={scene.sceneRevision}
						onError={onError}
					/>
				</div>
			) : null}
		</aside>
	);
}
