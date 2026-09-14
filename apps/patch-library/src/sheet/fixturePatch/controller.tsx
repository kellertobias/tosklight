import {
	createContext,
	type PropsWithChildren,
	type MouseEvent as ReactMouseEvent,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { type PatchHost, usePatchHost } from "../../host";
import { usePatch, usePatchView } from "../../state/PatchContext";
import type { PatchedFixture } from "../../wire";
import { parsePatchAddress } from "../fields";
import {
	fixtureDefinitionKey,
	mergeFixtureDefinitions,
} from "../fixtureProfileModel";
import {
	compareFixtureManufacturers,
	groupFixtureFamilies,
	isDmxPatchable,
} from "../patchUtils";
import {
	PATCH_SHEET_COLUMNS,
	type PatchSheetColumn,
	parseHiddenColumns,
} from "./patchColumns";
import { definitionSplits } from "./patchModel";
import { fixtureSelectionIds, usePatchSelection } from "./selection";
import { DEFAULT_PATCH_SORT, type PatchSort, sortPatchFixtures } from "./tableSort";

export type EditKind =
	| "number"
	| "name"
	| "note"
	| "address"
	| "location"
	| "rotation"
	| "mode"
	| "mib"
	| "masters"
	| "invert_pan"
	| "invert_tilt"
	| "bracket_angle"
	| "shaper_angle"
	| "scenery_width"
	| "scenery_height"
	| "scenery_depth"
	| "scenery_colour"
	| "chain_top"
	| "chain_bottom"
	| null;

export type VectorAxis = "x" | "y" | "z";
export type EditPresentation = "modal" | "inline" | "value_entry";

export type MultiPatchEdit = {
	fixtureId: string;
	instanceId: string;
	kind:
		| "address"
		| "location"
		| "rotation"
		| "invert_pan"
		| "invert_tilt"
		| "bracket_angle"
		| "shaper_angle";
	axis?: VectorAxis;
} | null;

export type PlacementBaseline = {
	draft: { name: string; fixtureNumber: string; count: string; patch: string };
	splitDrafts: Record<number, string>;
	definitionKey: string;
	empty: boolean;
};

export type FixturePatchSetupProps = {
	active?: boolean;
	title?: string;
	scope?: PatchFixtureScope;
	onMedia?: () => void;
	stagePreviewOpen?: boolean;
	stagePreviewClearance?: number;
	onStagePreview?: () => void;
	onOpenStageWindow?: () => void;
	addRequest?: number;
	/** External selection revisions use this to reveal entities hidden by a layer filter. */
	showAllLayersRequest?: number;
	initialTypeFilter?: string;
	/** Offers the Patch, Visualization and Compact column views in the title (Architect only). */
	quickViews?: boolean;
	/** Remembers the visible columns on this machine under this key; without it they reset. */
	columnStorageKey?: string;
	onFixturesAdded?: (
		fixtures: readonly { fixtureId: string; name: string }[],
	) => void | Promise<void>;
	/**
	 * Brings in a 3D model that is not in the fixture library and places it on `layerId`, resolving
	 * to the placed object's fixture ID, or `null` when the operator cancelled (Architect only).
	 */
	onImportVenueModel?: (layerId: string) => Promise<string | null>;
};

export type PatchFixtureScope = "all" | "dmx" | "venue" | "effects" | "media";

/** The sidebar entry for fixtures that belong to no stored layer of their own. */
export const NO_LAYER_ID = "__no_layer__";

/**
 * A fixture has a layer of its own only when it names a stored layer other than the implicit
 * default one that every new fixture starts on.
 */
export function fixtureHasAssignedLayer(
	fixture: Pick<PatchedFixture, "layer_id">,
	layerIds: ReadonlySet<string>,
) {
	const layerId = fixture.layer_id;
	return !!layerId && layerId !== "default" && layerIds.has(layerId);
}

function readHiddenColumns(key: string | undefined) {
	if (!key) return [];
	try {
		return parseHiddenColumns(localStorage.getItem(key));
	} catch {
		return [];
	}
}

function usePatchColumns(storageKey: string | undefined) {
	const [hidden, setHidden] = useState<readonly PatchSheetColumn[]>(() =>
		readHiddenColumns(storageKey),
	);
	const hiddenColumns = useMemo(() => new Set(hidden), [hidden]);
	const setHiddenColumns = (next: readonly PatchSheetColumn[]) => {
		// The last visible column stays: a table with no columns has nothing to select or edit.
		if (next.length >= PATCH_SHEET_COLUMNS.length) return;
		setHidden(next);
		if (!storageKey) return;
		try {
			localStorage.setItem(storageKey, JSON.stringify(next));
		} catch {
			// Unstorable columns still apply for as long as the window is open.
		}
	};
	return { hiddenColumns, setHiddenColumns };
}

function usePatchUiState() {
	const [activeLayer, setActiveLayer] = useState("all");
	const [showAllLayers, setShowAllLayers] = useState(false);
	const [selectedFixture, setSelectedFixture] = useState<string | null>(null);
	const [browserOpen, setBrowserOpen] = useState(false);
	const [placementOpen, setPlacementOpen] = useState(false);
	const [placementAddressOpen, setPlacementAddressOpen] = useState(false);
	const [layerModal, setLayerModal] = useState<"add" | "select" | null>(null);
	const [layerName, setLayerName] = useState("");
	const [query, setQuery] = useState("");
	const [sort, setSort] = useState<PatchSort>(DEFAULT_PATCH_SORT);
	const [typeFilter, setTypeFilter] = useState("");
	const [manufacturer, setManufacturer] = useState("");
	const [familyKey, setFamilyKey] = useState("");
	const [definitionKey, setDefinitionKey] = useState("");
	const [draft, setDraft] = useState({
		name: "Fixture 1",
		fixtureNumber: "1",
		count: "1",
		patch: "1.1",
	});
	const [splitDrafts, setSplitDrafts] = useState<Record<number, string>>({});
	const [batchPatches, setBatchPatches] = useState(["1.1"]);
	const [placementOverrides, setPlacementOverrides] = useState<
		Record<number, string>
	>({});
	const [placementEmpty, setPlacementEmpty] = useState(false);
	const [status, setStatus] = useState("");
	const [busy, setBusy] = useState(false);
	const [placementBaseline, setPlacementBaseline] =
		useState<PlacementBaseline | null>(null);
	const [placementCloseConfirm, setPlacementCloseConfirm] = useState(false);
	const [edit, setEdit] = useState<EditKind>(null);
	const [editPresentation, setEditPresentation] =
		useState<EditPresentation>("modal");
	const [editText, setEditText] = useState("");
	const [editBaseline, setEditBaseline] = useState("");
	const [editSplitDrafts, setEditSplitDrafts] = useState<
		Record<number, string>
	>({});
	const [editError, setEditError] = useState("");
	const [vector, setVector] = useState({ x: 0, y: 0, z: 0 });
	const [editAxis, setEditAxis] = useState<VectorAxis | null>(null);
	const [pending, setPending] = useState<Partial<PatchedFixture> | null>(null);
	const [blockedBy, setBlockedBy] = useState<PatchedFixture[]>([]);
	const [multipatchEdit, setMultipatchEdit] = useState<MultiPatchEdit>(null);
	const [editCloseConfirm, setEditCloseConfirm] = useState<
		"fixture" | "multipatch" | null
	>(null);
	const [deleteArmed, setDeleteArmed] = useState(false);
	const [deleteConfirm, setDeleteConfirm] = useState<
		readonly PatchedFixture[] | null
	>(null);
	const [editingSplit, setEditingSplit] = useState<number | null>(null);
	/** A fixture row the table scrolls into view once, if a committed edit moved it out of sight. */
	const [revealRequest, setRevealRequest] = useState<{
		fixtureId: string;
	} | null>(null);
	const selectionAnchor = useRef<string | null>(null);
	const dragSelection = useRef<string | null>(null);
	return {
		activeLayer,
		setActiveLayer,
		showAllLayers,
		setShowAllLayers,
		selectedFixture,
		setSelectedFixture,
		browserOpen,
		setBrowserOpen,
		placementOpen,
		setPlacementOpen,
		placementAddressOpen,
		setPlacementAddressOpen,
		layerModal,
		setLayerModal,
		layerName,
		setLayerName,
		query,
		setQuery,
		sort,
		setSort,
		typeFilter,
		setTypeFilter,
		manufacturer,
		setManufacturer,
		familyKey,
		setFamilyKey,
		definitionKey,
		setDefinitionKey,
		draft,
		setDraft,
		splitDrafts,
		setSplitDrafts,
		batchPatches,
		setBatchPatches,
		placementOverrides,
		setPlacementOverrides,
		placementEmpty,
		setPlacementEmpty,
		status,
		setStatus,
		busy,
		setBusy,
		placementBaseline,
		setPlacementBaseline,
		placementCloseConfirm,
		setPlacementCloseConfirm,
		edit,
		setEdit,
		editPresentation,
		setEditPresentation,
		editText,
		setEditText,
		editBaseline,
		setEditBaseline,
		editSplitDrafts,
		setEditSplitDrafts,
		editError,
		setEditError,
		vector,
		setVector,
		editAxis,
		setEditAxis,
		pending,
		setPending,
		blockedBy,
		setBlockedBy,
		multipatchEdit,
		setMultipatchEdit,
		editCloseConfirm,
		setEditCloseConfirm,
		deleteArmed,
		setDeleteArmed,
		deleteConfirm,
		setDeleteConfirm,
		editingSplit,
		setEditingSplit,
		revealRequest,
		setRevealRequest,
		selectionAnchor,
		dragSelection,
	};
}

function usePatchDerivedState(
	library: PatchHost["library"],
	patch: ReturnType<typeof usePatch>,
	ui: ReturnType<typeof usePatchUiState>,
	scope: PatchFixtureScope,
) {
	const all = [...patch.fixtures];
	const scoped = all.filter((fixture) =>
		definitionMatchesScope(fixture.definition, scope),
	);
	const layerIds = new Set(
		(library?.patchLayers ?? []).map((item) => item.body.id),
	);
	const unassigned = scoped.filter(
		(fixture) => !fixtureHasAssignedLayer(fixture, layerIds),
	);
	// Like any layer, the entry goes when nothing on this screen is in it, unless all are shown.
	const showUnassigned = ui.showAllLayers || unassigned.length > 0;
	const layers = [...(library?.patchLayers ?? [])]
		.sort((a, b) => a.body.order - b.body.order)
		.map((item) => item.body)
		.filter(
			(layer) => ui.showAllLayers || patchLayerIsVisible(layer.id, all, scope),
		);
	// Range selection walks this list, so it follows whatever order the table shows.
	const visible = sortPatchFixtures(
		scoped.filter(
			(fixture) =>
				ui.activeLayer === "all" ||
				(ui.activeLayer === NO_LAYER_ID
					? !fixtureHasAssignedLayer(fixture, layerIds)
					: (fixture.layer_id || "default") === ui.activeLayer),
		),
		ui.sort,
		{
			layerOrder: new Map(
				(library?.patchLayers ?? []).map((item) => [
					item.body.id,
					item.body.order,
				]),
			),
			note: (fixtureId) => library?.fixtureNotes?.get(fixtureId)?.note,
		},
	);
	const availableDefinitions = useMemo(
		() =>
			mergeFixtureDefinitions(library?.fixtureProfiles ?? [], [
				...(library?.fixtureLibrary ?? []),
				// A Venue model imported into this show lives in the show alone, so the show is where
				// another copy of it is found.
				...patch.fixtures
					.map((fixture) => fixture.definition)
					.filter((definition) => !isDmxPatchable(definition)),
			]).filter((definition) => definitionMatchesScope(definition, scope)),
		[library?.fixtureProfiles, library?.fixtureLibrary, patch.fixtures, scope],
	);
	const selected =
		all.find((fixture) => fixture.fixture_id === ui.selectedFixture) ?? null;
	const groupedDefinitions = useMemo(
		() => groupFixtureFamilies(availableDefinitions),
		[availableDefinitions],
	);
	const selectedModeFamily = selected
		? (groupedDefinitions.find(
				(item) =>
					item.manufacturer === selected.definition.manufacturer &&
					item.name === (selected.definition.name || selected.definition.model),
			) ?? null)
		: null;
	const types = useMemo(
		() =>
			[
				...new Set(
					availableDefinitions.map((item) => item.device_type || "other"),
				),
			].sort(),
		[availableDefinitions],
	);
	const filtered = useMemo(
		() =>
			filterDefinitions(availableDefinitions, {
				query: ui.query,
				typeFilter: ui.typeFilter,
				manufacturer: ui.manufacturer,
			}),
		[availableDefinitions, ui.query, ui.typeFilter, ui.manufacturer],
	);
	const families = useMemo(() => groupFixtureFamilies(filtered), [filtered]);
	const manufacturers = useMemo(
		() =>
			[...new Set(availableDefinitions.map((item) => item.manufacturer))].sort(
				compareFixtureManufacturers,
			),
		[availableDefinitions],
	);
	const family =
		families.find((item) => item.key === ui.familyKey) ?? families[0] ?? null;
	const definition =
		availableDefinitions.find(
			(item) => fixtureDefinitionKey(item) === ui.definitionKey,
		) ??
		family?.modes[0] ??
		null;
	const multipatchAddressFixture =
		ui.multipatchEdit?.kind === "address"
			? (all.find((item) => item.fixture_id === ui.multipatchEdit?.fixtureId) ??
				null)
			: null;
	const multipatchAddressInstance =
		multipatchAddressFixture?.multipatch?.find(
			(item) => item.id === ui.multipatchEdit?.instanceId,
		) ?? null;
	const previewPatch =
		definition && definitionSplits(definition).length > 1
			? (ui.splitDrafts[definitionSplits(definition)[0].number] ?? "")
			: ui.draft.patch;
	return {
		layers,
		unassigned,
		showUnassigned,
		all,
		scoped,
		visible,
		availableDefinitions,
		selected,
		selectedModeFamily,
		types,
		filtered,
		families,
		manufacturers,
		family,
		definition,
		multipatchAddressFixture,
		multipatchAddressInstance,
		shownUniverse: parsePatchAddress(previewPatch)?.universe ?? 1,
		shownAddress: parsePatchAddress(previewPatch)?.address ?? 0,
	};
}

const EFFECT_FIXTURE_TYPES = new Set(["effect", "fogger", "laser", "scenery"]);

export function definitionMatchesScope(
	definition: PatchedFixture["definition"],
	scope: PatchFixtureScope,
) {
	if (scope === "all") return true;
	if (scope === "media")
		return definition.device_type.trim().toLowerCase() === "media_server";
	const dmx = isDmxPatchable(definition);
	const effect = EFFECT_FIXTURE_TYPES.has(
		definition.device_type.trim().toLowerCase(),
	);
	if (scope === "venue") return !dmx;
	if (scope === "effects") return dmx && effect;
	return dmx && !effect;
}

export function patchLayerIsVisible(
	layerId: string,
	fixtures: readonly Pick<PatchedFixture, "layer_id" | "definition">[],
	scope: PatchFixtureScope,
) {
	const members = fixtures.filter(
		(fixture) => (fixture.layer_id || "default") === layerId,
	);
	return (
		members.length === 0 ||
		members.some((fixture) => definitionMatchesScope(fixture.definition, scope))
	);
}

function filterDefinitions(
	definitions: ReturnType<typeof mergeFixtureDefinitions>,
	ui: Pick<
		ReturnType<typeof usePatchUiState>,
		"query" | "typeFilter" | "manufacturer"
	>,
) {
	const needle = ui.query.trim().toLowerCase();
	return definitions.filter(
		(item) =>
			(!ui.typeFilter || item.device_type === ui.typeFilter) &&
			(!ui.manufacturer || item.manufacturer === ui.manufacturer) &&
			(!needle ||
				`${item.manufacturer} ${item.name} ${item.model} ${item.mode} ${item.device_type}`
					.toLowerCase()
					.includes(needle)),
	);
}

function useFixturePatchController(props: FixturePatchSetupProps) {
	const host = usePatchHost();
	const patch = usePatch();
	usePatchView(props.active ?? true);
	const selection = usePatchSelection();
	const ui = usePatchUiState();
	const columns = usePatchColumns(props.columnStorageKey);
	const handledAddRequest = useRef(0);
	// A request made before this sheet was mounted is already history: opening the sheet again must
	// not switch Show all back on for a selection the operator made on another screen long ago.
	const handledShowAllLayersRequest = useRef(props.showAllLayersRequest ?? 0);
	const data = usePatchDerivedState(
		host.library,
		patch,
		ui,
		props.scope ?? "all",
	);
	useEffect(() => {
		const request = props.showAllLayersRequest ?? 0;
		if (!request || request === handledShowAllLayersRequest.current) return;
		handledShowAllLayersRequest.current = request;
		// A selection made elsewhere is revealed, but an open layer that already holds all of it stays
		// open: leaving it would only take the operator away from where they are working.
		const selectedIds = selection.fixtureIds;
		if (ui.activeLayer !== "all" && selectedIds?.size) {
			const inLayer = new Set(data.visible.flatMap(fixtureSelectionIds));
			if ([...selectedIds].every((id) => inLayer.has(id))) return;
		}
		ui.setShowAllLayers(true);
		ui.setActiveLayer("all");
	}, [props.showAllLayersRequest, ui.setActiveLayer, ui.setShowAllLayers]);
	useEffect(() => {
		const request = props.addRequest ?? 0;
		if (!request || request === handledAddRequest.current) return;
		handledAddRequest.current = request;
		ui.setQuery("");
		ui.setManufacturer("");
		ui.setFamilyKey("");
		ui.setDefinitionKey("");
		ui.setTypeFilter(props.initialTypeFilter ?? "");
		ui.setBrowserOpen(true);
	}, [
		props.addRequest,
		props.initialTypeFilter,
		ui.setBrowserOpen,
		ui.setDefinitionKey,
		ui.setFamilyKey,
		ui.setManufacturer,
		ui.setQuery,
		ui.setTypeFilter,
	]);
	useEffect(() => {
		if (ui.activeLayer === "all") return;
		if (ui.activeLayer === NO_LAYER_ID ? data.showUnassigned : data.layers.some((layer) => layer.id === ui.activeLayer))
			return;
		ui.setActiveLayer("all");
	}, [data.layers, data.showUnassigned, ui.activeLayer, ui.setActiveLayer]);
	useEffect(() => {
		if (!data.family) return;
		if (
			!data.family.modes.some(
				(item) => fixtureDefinitionKey(item) === ui.definitionKey,
			)
		)
			ui.setDefinitionKey(fixtureDefinitionKey(data.family.modes[0]));
	}, [data.family, ui.definitionKey, ui.setDefinitionKey]);
	return {
		host,
		library: host.library,
		patch,
		selection,
		editArmed: host.editArmed,
		ui,
		data,
		columns,
		props: {
			title: props.title ?? "Show Patch",
			scope: props.scope ?? "all",
			onMedia: props.onMedia,
			stagePreviewOpen: props.stagePreviewOpen ?? false,
			stagePreviewClearance: props.stagePreviewClearance ?? 0,
			onStagePreview: props.onStagePreview,
			onOpenStageWindow: props.onOpenStageWindow,
			onFixturesAdded: props.onFixturesAdded,
			onImportVenueModel: props.onImportVenueModel,
			quickViews: props.quickViews ?? false,
		},
	};
}

export type PatchController = ReturnType<typeof useFixturePatchController>;

const PatchControllerContext = createContext<PatchController | null>(null);

export function PatchControllerProvider({
	children,
	...props
}: PropsWithChildren<FixturePatchSetupProps>) {
	const controller = useFixturePatchController(props);
	return (
		<PatchControllerContext.Provider value={controller}>
			{children}
		</PatchControllerContext.Provider>
	);
}

export function usePatchController() {
	const controller = useContext(PatchControllerContext);
	if (!controller)
		throw new Error(
			"usePatchController must be used inside PatchControllerProvider",
		);
	return controller;
}

export type PatchRowMouseEvent = ReactMouseEvent<HTMLTableRowElement>;
