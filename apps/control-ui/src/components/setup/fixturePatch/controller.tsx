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
import { useServer } from "../../../api/ServerContext";
import type { PatchedFixture } from "../../../api/types";
import { usePatch } from "../../../features/patch/PatchContext";
import { useApp } from "../../../state/AppContext";
import { parsePatchAddress } from "../../input/ConsoleFields";
import {
	fixtureDefinitionKey,
	mergeFixtureDefinitions,
} from "../fixtureProfileModel";
import {
	compareFixtureManufacturers,
	groupFixtureFamilies,
} from "../patchUtils";
import { compareFixtureIds } from "./fixtureIds";
import { definitionSplits } from "./patchModel";

export type EditKind =
	| "number"
	| "name"
	| "address"
	| "location"
	| "rotation"
	| "mode"
	| "mib"
	| "mib_delay"
	| "highlight"
	| null;

export type MultiPatchEdit = {
	fixtureId: string;
	instanceId: string;
	kind: "address" | "location" | "rotation";
} | null;

export type PlacementBaseline = {
	draft: { name: string; fixtureNumber: string; count: string; patch: string };
	splitDrafts: Record<number, string>;
	definitionKey: string;
};

export type FixturePatchSetupProps = {
	onMedia?: () => void;
	stagePreviewOpen?: boolean;
	stagePreviewClearance?: number;
	onStagePreview?: () => void;
};

function usePatchUiState() {
	const [activeLayer, setActiveLayer] = useState("all");
	const [selectedFixture, setSelectedFixture] = useState<string | null>(null);
	const [browserOpen, setBrowserOpen] = useState(false);
	const [placementOpen, setPlacementOpen] = useState(false);
	const [layerModal, setLayerModal] = useState<"add" | "select" | null>(null);
	const [layerName, setLayerName] = useState("");
	const [query, setQuery] = useState("");
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
	const [status, setStatus] = useState("");
	const [busy, setBusy] = useState(false);
	const [placementBaseline, setPlacementBaseline] =
		useState<PlacementBaseline | null>(null);
	const [placementCloseConfirm, setPlacementCloseConfirm] = useState(false);
	const [edit, setEdit] = useState<EditKind>(null);
	const [editText, setEditText] = useState("");
	const [editSplitDrafts, setEditSplitDrafts] = useState<
		Record<number, string>
	>({});
	const [editError, setEditError] = useState("");
	const [vector, setVector] = useState({ x: 0, y: 0, z: 0 });
	const [pending, setPending] = useState<Partial<PatchedFixture> | null>(null);
	const [blockedBy, setBlockedBy] = useState<PatchedFixture[]>([]);
	const [highlightDrafts, setHighlightDrafts] = useState<
		Record<string, string>
	>({});
	const [multipatchEdit, setMultipatchEdit] = useState<MultiPatchEdit>(null);
	const [editCloseConfirm, setEditCloseConfirm] = useState<
		"fixture" | "multipatch" | null
	>(null);
	const [deleteArmed, setDeleteArmed] = useState(false);
	const [deleteConfirm, setDeleteConfirm] = useState<PatchedFixture | null>(
		null,
	);
	const [editingSplit, setEditingSplit] = useState<number | null>(null);
	const selectionAnchor = useRef<string | null>(null);
	return {
		activeLayer,
		setActiveLayer,
		selectedFixture,
		setSelectedFixture,
		browserOpen,
		setBrowserOpen,
		placementOpen,
		setPlacementOpen,
		layerModal,
		setLayerModal,
		layerName,
		setLayerName,
		query,
		setQuery,
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
		editText,
		setEditText,
		editSplitDrafts,
		setEditSplitDrafts,
		editError,
		setEditError,
		vector,
		setVector,
		pending,
		setPending,
		blockedBy,
		setBlockedBy,
		highlightDrafts,
		setHighlightDrafts,
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
		selectionAnchor,
	};
}

function usePatchDerivedState(
	server: ReturnType<typeof useServer>,
	patch: ReturnType<typeof usePatch>,
	ui: ReturnType<typeof usePatchUiState>,
) {
	const layers = [...server.patchLayers]
		.sort((a, b) => a.body.order - b.body.order)
		.map((item) => item.body);
	const all = [...patch.fixtures];
	const visible = all
		.filter(
			(fixture) =>
				ui.activeLayer === "all" ||
				(fixture.layer_id || "default") === ui.activeLayer,
		)
		.sort(compareFixtureIds);
	const availableDefinitions = useMemo(
		() =>
			mergeFixtureDefinitions(server.fixtureProfiles, server.fixtureLibrary),
		[server.fixtureProfiles, server.fixtureLibrary],
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
		() => filterDefinitions(availableDefinitions, ui),
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
		all,
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
	const server = useServer();
	const patch = usePatch();
	const app = useApp();
	const ui = usePatchUiState();
	const data = usePatchDerivedState(server, patch, ui);
	useEffect(() => {
		if (!data.family) return;
		if (
			!data.family.modes.some(
				(item) => fixtureDefinitionKey(item) === ui.definitionKey,
			)
		)
			ui.setDefinitionKey(fixtureDefinitionKey(data.family.modes[0]));
	}, [data.family, ui.definitionKey]);
	return {
		server,
		patch,
		appState: app.state,
		dispatch: app.dispatch,
		ui,
		data,
		props: {
			onMedia: props.onMedia,
			stagePreviewOpen: props.stagePreviewOpen ?? false,
			stagePreviewClearance: props.stagePreviewClearance ?? 0,
			onStagePreview: props.onStagePreview,
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
