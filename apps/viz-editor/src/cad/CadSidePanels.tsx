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
import { duplicateSelection } from "./cadDuplicate";
import { CadObjectMenu, type CadObjectMenuRequest } from "./CadObjectMenu";
import {
	DeleteSelectionButton,
	type SelectedElement,
	selectedElements,
	useDeleteSelection,
} from "./CadDeleteSelection";
import { CadInfoPanel, type InfoTab } from "./CadInfoPanel";
import { SelectedElementList, SeveralPlacement } from "./CadInfoSeveral";
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
							<button
								type="button"
								aria-label={`${index + 1}. ${page.name}`}
								aria-pressed={page.id === selectedId}
								onClick={() => printPages.select(page.id)}
							>
								<span className="cad-print-row-number">{index + 1}</span>
								<strong title={page.name}>{page.name}</strong>
								<small className="cad-print-row-view">
									{page.kind === "fixture_list" ? "Fixture table" : CAD_VIEW_LABELS[page.view]}
								</small>
								<small className="cad-print-row-paper">
									A4 {page.orientation === "landscape" ? "Landscape" : "Portrait"}
								</small>
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

/** Every placement of the one fixture the selection names: the fixture itself first, then its copies. */
function selectedPlacements(scene: CadSceneSnapshot | null): CadEntity[] {
	if (!scene || scene.selectedIds.length !== 1) return [];
	const id = scene.selectedIds[0];
	const seen = new Set<string>();
	return scene.entities
		.filter((entity) => entity.logicalFixtureId === id && !seen.has(entity.id) && seen.add(entity.id))
		.sort((a, b) => Number(b.id === id) - Number(a.id === id));
}

/** The placement Info edits: the copy last clicked when it belongs to the selection, else the fixture. */
function selectedEntity(placements: readonly CadEntity[], focusedId: string | null): CadEntity | null {
	return placements.find((entity) => entity.id === focusedId) ?? placements[0] ?? null;
}

const addGroup = (label: string, items: TitleDropdownItem[]): TitleActionGroup => ({
	id: "panel-add",
	actions: [
		{
			id: "add",
			kind: "dropdown",
			icon: (
				<svg className="cad-sidebar-add-icon" viewBox="0 0 16 16" aria-hidden="true">
					<path d="M8 2.5v11M2.5 8h11" />
				</svg>
			),
			ariaLabel: label,
			dropdown: { kind: "items", ariaLabel: label, items },
		},
	],
});

/** Info's **Generic** and **Placement** tabs. */
function infoTabsGroup(active: InfoTab, onChange: (tab: InfoTab) => void): TitleActionGroup {
	return {
		id: "info-tabs",
		kind: "tabs",
		activeId: active,
		onActiveChange: (id) => onChange(id as InfoTab),
		actions: [
			{ id: "generic", label: "Generic" },
			{ id: "placement", label: "Placement" },
		],
	};
}

/** What the side panel's title row carries: the open panel's tabs and add menu, or Info's tabs. */
function titleGroups({
	panel,
	tab,
	onTab,
	tools,
	request,
	printPages,
	infoTabs,
}: {
	panel: CadPanel;
	tab: ElementsTab;
	onTab(tab: ElementsTab): void;
	tools: CadTools;
	request(kind: keyof ElementsRequests): void;
	printPages: PrintPages;
	infoTabs: TitleActionGroup;
}): TitleActionGroup[] {
	if (panel === "elements")
		return [
			{
				id: "elements-tabs",
				kind: "tabs",
				activeId: tab,
				onActiveChange: (id) => onTab(id as ElementsTab),
				actions: [
					{ id: "drawings", label: "Drawings" },
					{ id: "objects", label: "Objects" },
				],
			},
			addGroup(tab === "drawings" ? "Add drawing" : "Add object", elementsAddItems(tab, tools, request)),
		];
	if (panel === "print")
		return [
			addGroup("Add plan page", [
				{ kind: "action", id: "fixture-list", label: "Fixture list", onPress: printPages.addFixtureList },
			]),
		];
	return [infoTabs];
}

/** The right-click menu of the selection, while it is open and something is selected. */
function SelectionMenu({
	objectMenu,
	elements,
	onDelete,
	onSelect,
	onFocusEntity,
	onError,
}: {
	objectMenu: ObjectMenuState | undefined;
	elements: readonly SelectedElement[];
	onDelete(): void;
	onSelect(ids: string[]): void;
	onFocusEntity(entityId: string | null): void;
	onError(reason: unknown): void;
}) {
	const request = objectMenu?.request;
	if (!objectMenu || !request || !elements.length) return null;
	const duplicate = () =>
		duplicateSelection(
			elements.map((element) => element.id),
			request.duplicateOffset,
		)
			.then((ids) => {
				if (!ids.length) return;
				// The copies become the selection, so the next move or delete is theirs alone.
				onFocusEntity(ids[0]);
				onSelect(ids);
			})
			.catch(onError);
	return (
		<CadObjectMenu
			request={request}
			count={elements.length}
			onClose={objectMenu.close}
			onDelete={onDelete}
			onDuplicate={() => void duplicate()}
		/>
	);
}

interface ObjectMenuState {
	request: CadObjectMenuRequest | null;
	close(): void;
}

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
	focusedEntityId,
	onFocusEntity,
	objectMenu,
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
	/** The placement last clicked, which picks the copy Info edits. */
	focusedEntityId: string | null;
	onFocusEntity(entityId: string | null): void;
	/** The right-click menu of the selection, which runs its Delete through the same confirmation. */
	objectMenu?: ObjectMenuState;
	onError(reason: unknown): void;
}) {
	const [width, setWidth] = useStoredWidth();
	const [tab, setTab] = useState<ElementsTab>("drawings");
	// Kept while the selection changes, so stepping through lamps stays on the same tab.
	const [infoTab, setInfoTab] = useState<InfoTab>("generic");
	const [requests, setRequests] = useState<ElementsRequests>({
		newFolder: 0,
		chooseDrawing: 0,
		importModel: 0,
	});
	const selectionCount = scene?.selectedIds.length ?? 0;
	const placements = selectedPlacements(scene);
	const elements = scene ? selectedElements(scene.entities, scene.selectedIds) : [];
	const deletion = useDeleteSelection({
		elements,
		onDeleted: () => onSelect([]),
		onError,
	});
	const overlays = (
		<>
			{deletion.dialog}
			<SelectionMenu {...{ objectMenu, elements, onSelect, onFocusEntity, onError }} onDelete={deletion.request} />
		</>
	);
	if (!panel && selectionCount === 0) return overlays;
	const deleteButton = (
		<DeleteSelectionButton count={elements.length} onPress={(event) => deletion.request(event)} />
	);

	const infoTabs = infoTabsGroup(infoTab, setInfoTab);
	const request = (kind: keyof ElementsRequests) =>
		setRequests((current) => ({ ...current, [kind]: current[kind] + 1 }));
	const title = panel === "print" ? "Plans" : panel === "elements" ? "Elements" : "Info";
	const groups = titleGroups({ panel, tab, onTab: setTab, tools, request, printPages, infoTabs });

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
				{/* With no panel open this row is Info's own title, so its trash button sits here. */}
				{panel ? null : deleteButton}
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
						entity={selectedEntity(placements, focusedEntityId)}
						placements={placements}
						onChoosePlacement={onFocusEntity}
						selectionCount={selectionCount}
						sceneRevision={scene.sceneRevision}
						onError={onError}
						tab={infoTab}
						action={
							panel ? (
								<>
									<TitleChrome
										className="ui-window-action-groups"
										groupClassName="ui-window-action-group"
										terminalActions={[]}
										groups={[infoTabs]}
									/>
									{deleteButton}
								</>
							) : null
						}
						several={
							elements.length < 2 ? undefined : infoTab === "generic" ? (
								<SelectedElementList elements={elements} onSelect={(id) => onSelect([id])} />
							) : (
								<SeveralPlacement
									elements={elements}
									sceneRevision={scene.sceneRevision}
									onError={onError}
								/>
							)
						}
					/>
				</div>
			) : null}
			{overlays}
		</aside>
	);
}
