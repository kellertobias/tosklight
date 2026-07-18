import {
	Fragment,
	useEffect,
	useMemo,
	useRef,
	useState,
	type MouseEvent as ReactMouseEvent,
} from "react";
import { useServer } from "../../api/ServerContext";
import type {
	FixtureChannel,
	FixtureDefinition,
	MultiPatchInstance,
	PatchedFixture,
	SplitPatch,
} from "../../api/types";
import { useApp } from "../../state/AppContext";
import { SearchBar } from "../common/SearchBar";
import {
	ConsoleNumberField,
	ConsoleTextField,
	parsePatchAddress,
} from "../input/ConsoleFields";
import {
	compatibleHighlightOverrides,
	compareFixtureManufacturers,
	conflicts,
	firstFreeAddress,
	fixtureRange,
	fixtureRanges,
	groupFixtureFamilies,
	incrementFixtureName,
	isDmxPatchable,
} from "./patchUtils";
import {
	Button,
	ModalTitleBar,
	NumberField,
	Select,
	TextInput,
} from "../common";
import { WindowHeader } from "../window-kit";
import {
	fixtureDefinitionKey,
	maxRaw,
	mergeFixtureDefinitions,
} from "./fixtureProfileModel";

type EditKind =
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
type MultiPatchEdit = {
	fixtureId: string;
	instanceId: string;
	kind: "address" | "location" | "rotation";
} | null;
type PlacementBaseline = {
	draft: { name: string; fixtureNumber: string; count: string; patch: string };
	splitDrafts: Record<number, string>;
	definitionKey: string;
};

export function FixturePatchSetup({
	onMedia,
	stagePreviewOpen = false,
	stagePreviewClearance = 0,
	onStagePreview,
}: {
	onMedia?: () => void;
	stagePreviewOpen?: boolean;
	stagePreviewClearance?: number;
	onStagePreview?: () => void;
} = {}) {
	const server = useServer();
	const { state, dispatch } = useApp();
	const layers = [...server.patchLayers]
		.sort((a, b) => a.body.order - b.body.order)
		.map((item) => item.body);
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
	const all = server.patch?.fixtures ?? [];
	const visible = all
		.filter(
			(fixture) =>
				activeLayer === "all" ||
				(fixture.layer_id || "default") === activeLayer,
		)
		.sort(compareFixtureIds);
	const availableDefinitions = useMemo(
		() =>
			mergeFixtureDefinitions(server.fixtureProfiles, server.fixtureLibrary),
		[server.fixtureProfiles, server.fixtureLibrary],
	);
	const selected =
		all.find((fixture) => fixture.fixture_id === selectedFixture) ?? null;
	const selectedModeFamily = selected
		? (groupFixtureFamilies(availableDefinitions).find(
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
			availableDefinitions.filter((item) => {
				const needle = query.trim().toLowerCase();
				return (
					(!typeFilter || item.device_type === typeFilter) &&
					(!manufacturer || item.manufacturer === manufacturer) &&
					(!needle ||
						`${item.manufacturer} ${item.name} ${item.model} ${item.mode} ${item.device_type}`
							.toLowerCase()
							.includes(needle))
				);
			}),
		[availableDefinitions, query, typeFilter, manufacturer],
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
		families.find((item) => item.key === familyKey) ?? families[0] ?? null;
	const definition =
		availableDefinitions.find(
			(item) => fixtureDefinitionKey(item) === definitionKey,
		) ??
		family?.modes[0] ??
		null;
	useEffect(() => {
		if (!family) return;
		if (
			!family.modes.some((item) => fixtureDefinitionKey(item) === definitionKey)
		)
			setDefinitionKey(fixtureDefinitionKey(family.modes[0]));
	}, [family, definitionKey]);
	const chooseFamily = (key: string) => {
		const next = families.find((item) => item.key === key);
		if (!next) return;
		setFamilyKey(key);
		setDefinitionKey(fixtureDefinitionKey(next.modes[0]));
		setDraft((current) => ({ ...current, name: next.name }));
	};
	const beginPlacement = () => {
		if (!definition) return;
		if (!isDmxPatchable(definition)) {
			const used = new Set(
				all.flatMap((fixture) =>
					fixture.virtual_fixture_number == null
						? []
						: [fixture.virtual_fixture_number],
				),
			);
			const first = nextAvailableFixtureNumber(1, used) ?? 1;
			const nextDraft = {
				...draft,
				fixtureNumber: `0.${first}`,
				patch: "",
				name: draft.name || definition.name,
			};
			setDraft(nextDraft);
			setSplitDrafts({});
			setBatchPatches([]);
			setPlacementBaseline({
				draft: nextDraft,
				splitDrafts: {},
				definitionKey: fixtureDefinitionKey(definition),
			});
			setPlacementCloseConfirm(false);
			setStatus("");
			setPlacementOpen(true);
			return;
		}
		const splits = definitionSplits(definition);
		const universe = parsePatchAddress(draft.patch)?.universe ?? 1;
		const address =
			firstFreeAddress(
				all,
				universe,
				splits[0]?.footprint ?? definition.footprint,
			) ?? 1;
		const nextDraft = {
			...draft,
			patch: `${universe}.${address}`,
			name: draft.name || definition.name,
		};
		const nextSplitDrafts = Object.fromEntries(
			splits.map((split, index) => {
				const splitAddress =
					firstFreeAddress(
						all,
						universe,
						split.footprint,
						index === 0 ? address : undefined,
					) ?? 1;
				return [split.number, `${universe}.${splitAddress}`];
			}),
		);
		setDraft(nextDraft);
		setSplitDrafts(nextSplitDrafts);
		setBatchPatches(
			contiguousBatchPatches(
				universe,
				address,
				placementBatchCount(draft.count),
				splits[0]?.footprint ?? definition.footprint,
			),
		);
		setPlacementBaseline({
			draft: nextDraft,
			splitDrafts: nextSplitDrafts,
			definitionKey: fixtureDefinitionKey(definition),
		});
		setPlacementCloseConfirm(false);
		setStatus("");
		setPlacementOpen(true);
	};
	const updatePlacementCount = (count: string) => {
		setDraft((current) => ({ ...current, count }));
		if (!definition || !isDmxPatchable(definition)) return;
		const footprint =
			definitionSplits(definition)[0]?.footprint ?? definition.footprint;
		const base = parsePatchAddress(batchPatches[0] ?? draft.patch);
		if (!base) return;
		setBatchPatches((current) =>
			resizeBatchPatches(
				current,
				placementBatchCount(count),
				base.universe,
				base.address,
				footprint,
			),
		);
	};
	const updatePlacementPatch = (patch: string) => {
		setDraft((current) => ({ ...current, patch }));
		if (!definition) return;
		const parsed = parsePatchAddress(patch);
		if (parsed)
			setBatchPatches(
				contiguousBatchPatches(
					parsed.universe,
					parsed.address,
					placementBatchCount(draft.count),
					definitionSplits(definition)[0]?.footprint ?? definition.footprint,
				),
			);
	};
	const updateSplitPlacementPatch = (split: number, value: string) => {
		setSplitDrafts((current) => ({ ...current, [split]: value }));
		if (!definition || split !== definitionSplits(definition)[0]?.number)
			return;
		const parsed = parsePatchAddress(value);
		if (parsed) {
			setDraft((current) => ({ ...current, patch: value }));
			setBatchPatches(
				contiguousBatchPatches(
					parsed.universe,
					parsed.address,
					placementBatchCount(draft.count),
					definitionSplits(definition)[0].footprint,
				),
			);
		}
	};
	const updateBatchPatch = (
		index: number,
		universe: number,
		address: number,
	) => {
		const value = `${universe}.${address}`;
		setBatchPatches((current) =>
			current.map((patch, candidate) => (candidate === index ? value : patch)),
		);
		if (index === 0) {
			setDraft((current) => ({ ...current, patch: value }));
			const primarySplit =
				definition && definitionSplits(definition)[0]?.number;
			if (primarySplit != null)
				setSplitDrafts((current) => ({ ...current, [primarySplit]: value }));
		}
	};
	const addSplitBatch = async () => {
		if (!definition) return;
		const fixtureNumber = parseFixtureNumber(draft.fixtureNumber);
		if (fixtureNumber == null)
			return setStatus("Enter a positive whole-number start fixture ID.");
		const splits = definitionSplits(definition);
		const parsed = splits.map((split) => {
			const raw = splitDrafts[split.number]?.trim() ?? "";
			return { split, raw, address: raw ? parsePatchAddress(raw) : null };
		});
		if (parsed.some((item) => item.raw && !item.address))
			return setStatus(
				"Enter split patches as universe.address, for example 1.101.",
			);
		let remaining = placementBatchCount(draft.count),
			added = 0,
			lastId: string | null = null,
			fixtureNumberCursor = fixtureNumber;
		const usedFixtureNumbers = new Set(
			all.flatMap((fixture) =>
				fixture.fixture_number == null ? [] : [fixture.fixture_number],
			),
		);
		const addresses = parsed.map((item) => ({
			split: item.split,
			universe: item.address?.universe ?? null,
			address: item.address?.address ?? null,
		}));
		const initialError = splitPatchSetError(
			definition,
			addresses.map((item) => ({
				split: item.split.number,
				universe: item.universe,
				address: item.address,
			})),
		);
		if (initialError) return setStatus(initialError);
		setBusy(true);
		while (remaining > 0) {
			const plannedPrimary = parsePatchAddress(batchPatches[added] ?? "");
			if (plannedPrimary) {
				addresses[0].universe = plannedPrimary.universe;
				addresses[0].address = plannedPrimary.address;
			}
			if (
				splitPatchSetError(
					definition,
					addresses.map((item) => ({
						split: item.split.number,
						universe: item.universe,
						address: item.address,
					})),
				) ||
				!addresses.every(
					(item) =>
						item.address == null ||
						item.universe == null ||
						!conflicts(all, item.universe, item.address, item.split.footprint)
							.length,
				)
			)
				break;
			const nextFixtureNumber = nextAvailableFixtureNumber(
				fixtureNumberCursor,
				usedFixtureNumbers,
			);
			if (nextFixtureNumber == null) break;
			const split_patches: SplitPatch[] = addresses.map((item) => ({
				split: item.split.number,
				universe: item.universe,
				address: item.address,
			}));
			const primary =
				split_patches.find((item) => item.split === 1) ?? split_patches[0];
			lastId = await server.patchFixture({
				name: incrementFixtureName(draft.name, added),
				fixture_number: nextFixtureNumber,
				definition,
				universe: primary?.universe ?? null,
				address: primary?.address ?? null,
				split_patches,
				layer_id: activeLayer === "all" ? "default" : activeLayer,
			});
			if (!lastId) break;
			usedFixtureNumbers.add(nextFixtureNumber);
			fixtureNumberCursor = nextFixtureNumber + 1;
			added++;
			remaining--;
			addresses.forEach((item, index) => {
				if (index > 0 && item.address != null)
					item.address += item.split.footprint;
			});
		}
		setBusy(false);
		if (lastId) {
			setSelectedFixture(lastId);
			void server.setSelection([lastId]);
		}
		if (!remaining) {
			setPlacementOpen(false);
			setBrowserOpen(false);
			setStatus("");
			return;
		}
		const nextFixtureNumber = nextAvailableFixtureNumber(
			fixtureNumberCursor,
			usedFixtureNumbers,
		);
		const nextPatches = batchPatches.slice(added);
		setBatchPatches(nextPatches);
		setDraft((current) => ({
			...current,
			fixtureNumber: String(nextFixtureNumber ?? fixtureNumberCursor),
			count: String(remaining),
			patch: nextPatches[0] ?? current.patch,
		}));
		setSplitDrafts(
			Object.fromEntries(
				addresses.map((item) => [
					item.split.number,
					item.universe != null && item.address != null
						? `${item.universe}.${item.address}`
						: "",
				]),
			),
		);
		setStatus(
			`${added} fixture${added === 1 ? "" : "s"} added. Choose where to patch the remaining ${remaining}.`,
		);
	};
	const addBatch = async () => {
		if (!definition) return;
		if (!isDmxPatchable(definition)) {
			const fixtureNumber = parseVirtualFixtureNumber(draft.fixtureNumber);
			if (fixtureNumber == null)
				return setStatus("Enter a virtual fixture ID starting at 0.1.");
			let remaining = placementBatchCount(draft.count),
				added = 0,
				lastId: string | null = null,
				fixtureNumberCursor = fixtureNumber;
			const usedFixtureNumbers = new Set(
				all.flatMap((fixture) =>
					fixture.virtual_fixture_number == null
						? []
						: [fixture.virtual_fixture_number],
				),
			);
			setBusy(true);
			while (remaining > 0) {
				const nextFixtureNumber = nextAvailableFixtureNumber(
					fixtureNumberCursor,
					usedFixtureNumbers,
				);
				if (nextFixtureNumber == null) break;
				lastId = await server.patchFixture({
					name: incrementFixtureName(draft.name, added),
					fixture_number: null,
					virtual_fixture_number: nextFixtureNumber,
					definition,
					universe: null,
					address: null,
					split_patches: definitionSplits(definition).map((split) => ({
						split: split.number,
						universe: null,
						address: null,
					})),
					layer_id: activeLayer === "all" ? "default" : activeLayer,
				});
				if (!lastId) break;
				usedFixtureNumbers.add(nextFixtureNumber);
				fixtureNumberCursor = nextFixtureNumber + 1;
				added++;
				remaining--;
			}
			setBusy(false);
			if (lastId) setSelectedFixture(lastId);
			if (!remaining) {
				setPlacementOpen(false);
				setBrowserOpen(false);
				setStatus("");
			}
			return;
		}
		if (definitionSplits(definition).length > 1) return addSplitBatch();
		const fixtureNumber = parseFixtureNumber(draft.fixtureNumber);
		if (fixtureNumber == null)
			return setStatus("Enter a positive whole-number start fixture ID.");
		const planned = batchPatches
			.slice(0, placementBatchCount(draft.count))
			.map(parsePatchAddress);
		const plannedError = batchPatchError(planned, definition.footprint, all);
		if (plannedError) return setStatus(plannedError);
		let remaining = planned.length,
			added = 0,
			lastId: string | null = null,
			fixtureNumberCursor = fixtureNumber;
		const usedFixtureNumbers = new Set(
			all.flatMap((fixture) =>
				fixture.fixture_number == null ? [] : [fixture.fixture_number],
			),
		);
		setBusy(true);
		for (const patch of planned) {
			if (!patch) break;
			const nextFixtureNumber = nextAvailableFixtureNumber(
				fixtureNumberCursor,
				usedFixtureNumbers,
			);
			if (nextFixtureNumber == null) break;
			lastId = await server.patchFixture({
				name: incrementFixtureName(draft.name, added),
				fixture_number: nextFixtureNumber,
				definition,
				universe: patch.universe,
				address: patch.address,
				layer_id: activeLayer === "all" ? "default" : activeLayer,
			});
			if (!lastId) break;
			usedFixtureNumbers.add(nextFixtureNumber);
			fixtureNumberCursor = nextFixtureNumber + 1;
			added++;
			remaining--;
		}
		setBusy(false);
		if (lastId) {
			setSelectedFixture(lastId);
			void server.setSelection([lastId]);
		}
		if (!remaining) {
			setPlacementOpen(false);
			setBrowserOpen(false);
			setStatus("");
			return;
		}
		const nextFixtureNumber = nextAvailableFixtureNumber(
			fixtureNumberCursor,
			usedFixtureNumbers,
		);
		const nextPatches = batchPatches.slice(added);
		setBatchPatches(nextPatches);
		setDraft((current) => ({
			...current,
			fixtureNumber: String(nextFixtureNumber ?? fixtureNumberCursor),
			count: String(remaining),
			patch: nextPatches[0] ?? current.patch,
		}));
		setStatus(
			`${added} fixture${added === 1 ? "" : "s"} added. Choose where to patch the remaining ${remaining}.`,
		);
	};
	const armEdit = (fixture: PatchedFixture, kind: Exclude<EditKind, null>) => {
		if (!state.patchSetArmed) return;
		setEditError("");
		setSelectedFixture(fixture.fixture_id);
		if (kind === "number") setEditText(String(fixtureDisplayId(fixture)));
		else if (kind === "name")
			setEditText(fixture.name || fixture.definition.name);
		else if (kind === "address") {
			setEditText(
				fixture.universe && fixture.address
					? `${fixture.universe}.${fixture.address}`
					: "",
			);
			setEditSplitDrafts(
				Object.fromEntries(
					effectiveSplitPatches(
						fixture.definition,
						fixture.split_patches,
						fixture.universe,
						fixture.address,
					).map((patch) => [
						patch.split,
						patch.universe && patch.address
							? `${patch.universe}.${patch.address}`
							: "",
					]),
				),
			);
		} else if (kind === "mib")
			setEditText(String(fixture.move_in_black_enabled ?? true));
		else if (kind === "mib_delay")
			setEditText(String((fixture.move_in_black_delay_millis ?? 0) / 1000));
		else if (kind === "highlight")
			setHighlightDrafts(
				Object.fromEntries(
					Object.entries(fixture.highlight_overrides ?? {}).map(
						([channelId, raw]) => [channelId, String(raw)],
					),
				),
			);
		else if (kind === "location" || kind === "rotation")
			setVector(fixture[kind] ?? { x: 0, y: 0, z: 0 });
		else if (kind === "mode") {
			const fixtureFamily = groupFixtureFamilies(availableDefinitions).find(
				(item) =>
					item.manufacturer === fixture.definition.manufacturer &&
					item.name === (fixture.definition.name || fixture.definition.model),
			);
			if (fixtureFamily) {
				setFamilyKey(fixtureFamily.key);
				setDefinitionKey(fixtureDefinitionKey(fixture.definition));
			}
		}
		setEdit(kind);
	};
	const selectSplitAddress = (fixture: PatchedFixture, split: number) => {
		setSelectedFixture(fixture.fixture_id);
		if (!state.patchSetArmed) void server.setSelection([fixture.fixture_id]);
		setEditingSplit(split);
		setEditError("");
		setEditSplitDrafts(
			Object.fromEntries(
				effectiveSplitPatches(
					fixture.definition,
					fixture.split_patches,
					fixture.universe,
					fixture.address,
				).map((patch) => [
					patch.split,
					patch.universe && patch.address
						? `${patch.universe}.${patch.address}`
						: "",
				]),
			),
		);
		if (state.patchSetArmed) setEdit("address");
	};
	useEffect(() => {
		if (
			state.patchSetArmed &&
			selected &&
			editingSplit != null &&
			definitionSplits(selected.definition).length > 1
		)
			setEdit("address");
	}, [state.patchSetArmed, selected, editingSplit]);
	const finishEdit = async (changes: Partial<PatchedFixture>) => {
		if (!selected) return false;
		if (!(await server.updatePatchedFixture(selected.fixture_id, changes)))
			return false;
		setEdit(null);
		setEditingSplit(null);
		setPending(null);
		setBlockedBy([]);
		dispatch({ type: "SET_PATCH_ARMED", value: false });
		return true;
	};
	const applyEdit = async (changes: Partial<PatchedFixture>) => {
		if (!selected) return;
		setEditError("");
		const changesPatch =
			"definition" in changes ||
			"universe" in changes ||
			"address" in changes ||
			"split_patches" in changes ||
			"multipatch" in changes;
		if (changesPatch) {
			const candidate = { ...selected, ...changes };
			const owners = [
				{
					split_patches: candidate.split_patches,
					universe: candidate.universe,
					address: candidate.address,
				},
				...(candidate.multipatch ?? []),
			];
			for (const owner of owners) {
				const patches = effectiveSplitPatches(
					candidate.definition,
					owner.split_patches,
					owner.universe,
					owner.address,
				);
				const invalid = splitPatchSetError(candidate.definition, patches);
				if (invalid) {
					setEditError(invalid);
					return;
				}
			}
			const ranges = fixtureRanges(candidate);
			for (let index = 0; index < ranges.length; index++)
				for (let other = index + 1; other < ranges.length; other++) {
					const left = ranges[index];
					const right = ranges[other];
					if (
						left.universe === right.universe &&
						left.start <= right.end &&
						right.start <= left.end
					) {
						setEditError(
							`The fixture's split and multi-patch ranges overlap at universe ${left.universe}.`,
						);
						return;
					}
				}
			const found = all.filter(
				(fixture) =>
					fixture.fixture_id !== selected.fixture_id &&
					ranges.some((range) =>
						fixtureRanges(fixture).some(
							(other) =>
								other.universe === range.universe &&
								other.start <= range.end &&
								other.end >= range.start,
						),
					),
			);
			if (found.length) {
				setPending(changes);
				setBlockedBy(found);
				return;
			}
		}
		await finishEdit(changes);
	};
	const saveEdit = (value = editText) => {
		if (!selected) return;
		if (edit === "number") {
			if (isDmxPatchable(selected.definition)) {
				const number = parseFixtureNumber(value);
				if (
					number != null &&
					!all.some(
						(fixture) =>
							fixture.fixture_id !== selected.fixture_id &&
							fixture.fixture_number === number,
					)
				)
					void applyEdit({
						fixture_number: number,
						virtual_fixture_number: null,
					});
			} else {
				const number = parseVirtualFixtureNumber(value);
				if (
					number != null &&
					!all.some(
						(fixture) =>
							fixture.fixture_id !== selected.fixture_id &&
							fixture.virtual_fixture_number === number,
					)
				)
					void applyEdit({
						fixture_number: null,
						virtual_fixture_number: number,
					});
			}
		}
		if (edit === "name")
			void applyEdit({ name: value.trim() || selected.name });
		if (edit === "address") {
			const parsed = parsePatchAddress(value);
			if (selected.definition.schema_version >= 2) {
				const split = definitionSplits(selected.definition)[0]?.number ?? 1;
				if (parsed)
					void applyEdit(
						replaceSelectedSplitPatch(
							selected.definition,
							selected.split_patches,
							selected.universe,
							selected.address,
							split,
							parsed,
						),
					);
				else if (!value.trim())
					void applyEdit(
						replaceSelectedSplitPatch(
							selected.definition,
							selected.split_patches,
							selected.universe,
							selected.address,
							split,
							null,
						),
					);
			} else if (parsed) void applyEdit(parsed);
			else if (!value.trim()) void applyEdit({ universe: null, address: null });
		}
		if (edit === "mib")
			void applyEdit({ move_in_black_enabled: value === "true" });
		if (edit === "mib_delay") {
			const seconds = Number(value);
			if (Number.isFinite(seconds))
				void applyEdit({
					move_in_black_delay_millis: Math.max(0, Math.round(seconds * 1000)),
				});
		}
		if (edit === "location" || edit === "rotation")
			void applyEdit({ [edit]: vector });
		if (edit === "mode" && definition) {
			const highlight_overrides = compatibleHighlightOverrides(
				definition,
				selected.highlight_overrides,
			);
			void applyEdit({
				...reconcileModePatchChanges(selected, definition),
				highlight_overrides,
			});
		}
	};
	const saveHighlightEdit = () => {
		if (!selected) return;
		const channels = new Map(
			definitionModeChannels(selected.definition).map((channel) => [
				channel.id,
				channel,
			]),
		);
		const highlight_overrides: Record<string, number> = {};
		for (const [channelId, text] of Object.entries(highlightDrafts)) {
			if (!text.trim()) continue;
			const channel = channels.get(channelId);
			const raw = Number(text);
			if (
				!channel ||
				!Number.isInteger(raw) ||
				raw < 0 ||
				raw > maxRaw(channel.resolution)
			) {
				setEditError(
					`${channel?.attribute ?? "Highlight"} must be an exact raw value from 0 to ${channel ? maxRaw(channel.resolution) : 0}.`,
				);
				return;
			}
			highlight_overrides[channelId] = raw;
		}
		void applyEdit({ highlight_overrides });
	};
	const saveSplitEdit = () => {
		if (!selected) return;
		const parsed = definitionSplits(selected.definition).map((split) => {
			const raw = editSplitDrafts[split.number]?.trim() ?? "";
			return {
				split: split.number,
				raw,
				value: raw ? parsePatchAddress(raw) : null,
			};
		});
		if (parsed.some((item) => item.raw && !item.value)) return;
		const split_patches: SplitPatch[] = parsed.map((item) => ({
			split: item.split,
			universe: item.value?.universe ?? null,
			address: item.value?.address ?? null,
		}));
		const primary =
			split_patches.find((item) => item.split === 1) ?? split_patches[0];
		void applyEdit({
			split_patches,
			universe: primary?.universe ?? null,
			address: primary?.address ?? null,
		});
	};
	const saveSelectedSplitEdit = () => {
		if (!selected || editingSplit == null) return;
		const raw = editSplitDrafts[editingSplit]?.trim() ?? "";
		const value = raw ? parsePatchAddress(raw) : null;
		if (raw && !value) {
			setEditError(
				"Enter the split patch as universe.address, for example 1.101.",
			);
			return;
		}
		const changes = replaceSelectedSplitPatch(
			selected.definition,
			selected.split_patches,
			selected.universe,
			selected.address,
			editingSplit,
			value,
		);
		void applyEdit(changes);
	};
	const cancelEdit = () => {
		setEdit(null);
		setEditingSplit(null);
		setEditError("");
		setPending(null);
		setBlockedBy([]);
		dispatch({ type: "SET_PATCH_ARMED", value: false });
	};
	const fixtureVectorDirty = Boolean(
		selected &&
			(edit === "location" || edit === "rotation") &&
			JSON.stringify(vector) !==
				JSON.stringify(selected[edit] ?? { x: 0, y: 0, z: 0 }),
	);
	const multipatchVectorDirty = (() => {
		if (!multipatchEdit || multipatchEdit.kind === "address") return false;
		const fixture = all.find(
			(item) => item.fixture_id === multipatchEdit.fixtureId,
		);
		const instance = fixture?.multipatch?.find(
			(item) => item.id === multipatchEdit.instanceId,
		);
		return Boolean(
			instance &&
				JSON.stringify(vector) !==
					JSON.stringify(instance[multipatchEdit.kind]),
		);
	})();
	const requestFixtureEditClose = () =>
		fixtureVectorDirty ? setEditCloseConfirm("fixture") : cancelEdit();
	const closeMultipatchEdit = () => {
		setEditError("");
		setMultipatchEdit(null);
	};
	const requestMultipatchEditClose = () =>
		multipatchVectorDirty
			? setEditCloseConfirm("multipatch")
			: closeMultipatchEdit();
	const createLayer = async (value = layerName) => {
		const name = value.trim();
		if (!name) return;
		const id = crypto.randomUUID();
		if (await server.savePatchLayer({ id, name, order: layers.length })) {
			setActiveLayer(id);
			setLayerName("");
			setLayerModal(null);
		}
	};
	const selectLayer = async (layerId: string) => {
		if (
			selected &&
			(await server.updatePatchedFixture(selected.fixture_id, {
				layer_id: layerId,
			}))
		) {
			setLayerModal(null);
			dispatch({ type: "SET_PATCH_ARMED", value: false });
		}
	};
	const addMultipatch = async () => {
		if (!selected) return;
		const instance: MultiPatchInstance = {
			id: crypto.randomUUID(),
			name: "multi-patch",
			universe: null,
			address: null,
			split_patches: definitionSplits(selected.definition).map((split) => ({
				split: split.number,
				universe: null,
				address: null,
			})),
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
		};
		await server.updatePatchedFixture(selected.fixture_id, {
			multipatch: [...(selected.multipatch ?? []), instance],
		});
	};
	const beginMultipatchEdit = (
		fixture: PatchedFixture,
		instance: MultiPatchInstance,
		kind: NonNullable<MultiPatchEdit>["kind"],
	) => {
		setEditError("");
		setSelectedFixture(fixture.fixture_id);
		setMultipatchEdit({
			fixtureId: fixture.fixture_id,
			instanceId: instance.id,
			kind,
		});
		if (kind === "address") {
			setEditText(
				instance.universe && instance.address
					? `${instance.universe}.${instance.address}`
					: "",
			);
			setEditSplitDrafts(
				Object.fromEntries(
					effectiveSplitPatches(
						fixture.definition,
						instance.split_patches,
						instance.universe,
						instance.address,
					).map((patch) => [
						patch.split,
						patch.universe && patch.address
							? `${patch.universe}.${patch.address}`
							: "",
					]),
				),
			);
		} else setVector(instance[kind]);
	};
	const saveMultipatchEdit = async (value = editText) => {
		if (!multipatchEdit) return;
		setEditError("");
		const fixture = all.find(
			(item) => item.fixture_id === multipatchEdit.fixtureId,
		);
		const instance = fixture?.multipatch?.find(
			(item) => item.id === multipatchEdit.instanceId,
		);
		if (!fixture || !instance) return;
		let changes: Partial<MultiPatchInstance>;
		if (
			multipatchEdit.kind === "address" &&
			definitionSplits(fixture.definition).length > 1
		) {
			const parsed = definitionSplits(fixture.definition).map((split) => {
				const raw = editSplitDrafts[split.number]?.trim() ?? "";
				return {
					split: split.number,
					raw,
					value: raw ? parsePatchAddress(raw) : null,
				};
			});
			if (parsed.some((item) => item.raw && !item.value)) {
				setEditError(
					"Enter split patches as universe.address, for example 1.101.",
				);
				return;
			}
			const split_patches: SplitPatch[] = parsed.map((item) => ({
				split: item.split,
				universe: item.value?.universe ?? null,
				address: item.value?.address ?? null,
			}));
			const invalid = splitPatchSetError(fixture.definition, split_patches);
			if (invalid) {
				setEditError(invalid);
				return;
			}
			const primary =
				split_patches.find((patch) => patch.split === 1) ?? split_patches[0];
			changes = {
				split_patches,
				universe: primary?.universe ?? null,
				address: primary?.address ?? null,
			};
		} else if (multipatchEdit.kind === "address") {
			const parsed = parsePatchAddress(value);
			if (fixture.definition.schema_version >= 2) {
				const split = definitionSplits(fixture.definition)[0]?.number ?? 1;
				if (parsed)
					changes = replaceSelectedSplitPatch(
						fixture.definition,
						instance.split_patches,
						instance.universe,
						instance.address,
						split,
						parsed,
					);
				else if (!value.trim() || value.trim() === "0")
					changes = replaceSelectedSplitPatch(
						fixture.definition,
						instance.split_patches,
						instance.universe,
						instance.address,
						split,
						null,
					);
				else {
					setEditError(
						"Enter the patch as universe.address or clear it to unpatch.",
					);
					return;
				}
			} else if (parsed) changes = parsed;
			else if (!value.trim() || value.trim() === "0")
				changes = { universe: null, address: null };
			else {
				setEditError(
					"Enter the patch as universe.address or clear it to unpatch.",
				);
				return;
			}
		} else changes = { [multipatchEdit.kind]: vector };
		const multipatch = (fixture.multipatch ?? []).map((item) =>
			item.id === instance.id ? { ...item, ...changes } : item,
		);
		if (await server.updatePatchedFixture(fixture.fixture_id, { multipatch }))
			setMultipatchEdit(null);
	};
	const unpatchCurrentFixture = async () => {
		if (!selected) return;
		if (
			await server.updatePatchedFixture(
				selected.fixture_id,
				unpatchFixtureChanges(selected),
			)
		)
			cancelEdit();
	};
	const requestFixtureDelete = (fixture: PatchedFixture) => {
		setSelectedFixture(fixture.fixture_id);
		setDeleteConfirm(fixture);
		setDeleteArmed(false);
	};
	const deleteFixture = async () => {
		if (!deleteConfirm) return;
		if (await server.deletePatchedFixture(deleteConfirm.fixture_id)) {
			setDeleteConfirm(null);
			setDeleteArmed(false);
			if (selectedFixture === deleteConfirm.fixture_id) setSelectedFixture(null);
			cancelEdit();
		}
	};
	const unpatchFixtureFromDeleteConfirm = async () => {
		if (!deleteConfirm) return;
		if (
			await server.updatePatchedFixture(
				deleteConfirm.fixture_id,
				unpatchFixtureChanges(deleteConfirm),
			)
		) {
			setDeleteConfirm(null);
			setDeleteArmed(false);
			cancelEdit();
		}
	};
	const unpatchConflictsAndApply = async () => {
		if (
			!selected ||
			!pending ||
			!window.confirm("Unpatch the conflicting fixtures and apply this change?")
		)
			return;
		const cleared = await Promise.all(
			blockedBy.map((fixture) =>
				server.updatePatchedFixture(
					fixture.fixture_id,
					unpatchFixtureChanges(fixture),
				),
			),
		);
		if (cleared.some((result) => !result)) {
			setEditError(
				"One or more conflicting fixtures could not be unpatched. No new patch was applied.",
			);
			return;
		}
		await finishEdit(pending);
	};
	const setFixtureNumber = async (fixture: PatchedFixture) => {
		const visualOnly = !isDmxPatchable(fixture.definition);
		const value = window.prompt(
			"Fixture ID",
			String(fixtureDisplayId(fixture)),
		);
		if (value == null) return;
		if (visualOnly) {
			const number = parseVirtualFixtureNumber(value);
			if (number == null)
				return void window.alert("Visual fixture IDs must start at 0.1.");
			if (
				all.some(
					(candidate) =>
						candidate.fixture_id !== fixture.fixture_id &&
						candidate.virtual_fixture_number === number,
				)
			)
				return void window.alert(`Fixture ID 0.${number} is already in use.`);
			if (
				await server.updatePatchedFixture(fixture.fixture_id, {
					fixture_number: null,
					virtual_fixture_number: number,
				})
			)
				dispatch({ type: "SET_PATCH_ARMED", value: false });
			return;
		}
		const number = parseFixtureNumber(value);
		if (number == null)
			return void window.alert("Fixture IDs must be positive whole numbers.");
		if (
			all.some(
				(candidate) =>
					candidate.fixture_id !== fixture.fixture_id &&
					candidate.fixture_number === number,
			)
		)
			return void window.alert(`Fixture ID ${number} is already in use.`);
		if (
			await server.updatePatchedFixture(fixture.fixture_id, {
				fixture_number: number,
				virtual_fixture_number: null,
			})
		)
			dispatch({ type: "SET_PATCH_ARMED", value: false });
	};
	const placementDirty = Boolean(
		placementBaseline &&
			definition &&
			(placementBaseline.definitionKey !== fixtureDefinitionKey(definition) ||
				JSON.stringify(placementBaseline.draft) !== JSON.stringify(draft) ||
				JSON.stringify(placementBaseline.splitDrafts) !==
					JSON.stringify(splitDrafts)),
	);
	const closePlacement = () => {
		setPlacementOpen(false);
		setPlacementCloseConfirm(false);
		setPlacementBaseline(null);
		setStatus("");
	};
	const requestPlacementClose = () => {
		if (placementDirty) setPlacementCloseConfirm(true);
		else closePlacement();
	};
	const multipatchAddressFixture =
		multipatchEdit?.kind === "address"
			? (all.find((item) => item.fixture_id === multipatchEdit.fixtureId) ??
				null)
			: null;
	const multipatchAddressInstance =
		multipatchAddressFixture?.multipatch?.find(
			(item) => item.id === multipatchEdit?.instanceId,
		) ?? null;
	const selectPatchFixture = (
		fixture: PatchedFixture,
		event: ReactMouseEvent<HTMLTableRowElement>,
	) => {
		if (deleteArmed) {
			requestFixtureDelete(fixture);
			return;
		}
		setSelectedFixture(fixture.fixture_id);
		if (state.patchSetArmed) return;
		const ordered = visible.map((candidate) => candidate.fixture_id);
		if (event.shiftKey && selectionAnchor.current) {
			const from = ordered.indexOf(selectionAnchor.current);
			const to = ordered.indexOf(fixture.fixture_id);
			if (from >= 0 && to >= 0)
				void server.setSelection(
					ordered.slice(Math.min(from, to), Math.max(from, to) + 1),
				);
		} else if (event.ctrlKey || event.metaKey) {
			const current = new Set(server.selectedFixtures);
			const members = fixture.logical_heads.length
				? fixture.logical_heads.map((head) => head.fixture_id)
				: [fixture.fixture_id];
			if (members.every((member) => current.has(member))) {
				for (const member of members) current.delete(member);
			} else {
				for (const member of members) current.add(member);
			}
			void server.setSelection([...current]);
		} else {
			void server.setSelection([fixture.fixture_id]);
		}
		selectionAnchor.current = fixture.fixture_id;
	};
	useEffect(() => {
		const handlePatchDeleteKeys = (event: KeyboardEvent) => {
			if (event.repeat || event.metaKey || event.ctrlKey || event.altKey) return;
			const target = event.target as HTMLElement | null;
			const tag = target?.tagName;
			const textTarget =
				target?.isContentEditable ||
				tag === "INPUT" ||
				tag === "TEXTAREA" ||
				tag === "SELECT";
			if (deleteConfirm) {
				if (event.key === "Escape") {
					event.preventDefault();
					setDeleteConfirm(null);
					return;
				}
				if (event.key === "Enter") {
					event.preventDefault();
					void deleteFixture();
				}
				return;
			}
			if (
				textTarget ||
				edit ||
				multipatchEdit ||
				browserOpen ||
				placementOpen ||
				layerModal ||
				pending
			)
				return;
			if (event.key !== "Delete" && event.key !== "Backspace") return;
			const fixture = all.find((item) => item.fixture_id === selectedFixture);
			if (!fixture) return;
			event.preventDefault();
			requestFixtureDelete(fixture);
		};
		window.addEventListener("keydown", handlePatchDeleteKeys, true);
		return () =>
			window.removeEventListener("keydown", handlePatchDeleteKeys, true);
	}, [
		all,
		browserOpen,
		deleteConfirm,
		edit,
		layerModal,
		multipatchEdit,
		pending,
		placementOpen,
		selectedFixture,
	]);
	const previewPatch =
		definition && definitionSplits(definition).length > 1
			? (splitDrafts[definitionSplits(definition)[0].number] ?? "")
			: draft.patch;
	const shownUniverse = parsePatchAddress(previewPatch)?.universe ?? 1;
	const shownAddress = parsePatchAddress(previewPatch)?.address ?? 0;
	return (
		<div
			className={`show-patch-layout ${layerModal === "select" ? "layer-selecting" : ""}`}
		>
			<WindowHeader
				title="Show Patch"
				info={{
					primary: `${all.length} fixtures · ${layers.length} layers`,
					secondary: server.unresolvedMvrFixtures.length
						? `${server.unresolvedMvrFixtures.length} unresolved MVR fixtures excluded from output`
						: undefined,
				}}
				actions={[
					[
						...(onStagePreview
							? [
									{
										id: "preview-stage",
										label: "Preview Stage",
										active: stagePreviewOpen,
										onClick: onStagePreview,
									},
								]
							: []),
					],
					[
						...(onMedia
							? [
									{
										id: "fixtures",
										label: "Fixtures",
										active: true,
										onClick: () => undefined,
									},
									{ id: "media", label: "Media Servers", onClick: onMedia },
								]
							: []),
					],
					[
						...(selected && state.patchSetArmed
							? [
									{
										id: "fixture-number",
										label: "Set fixture ID",
										onClick: () => void setFixtureNumber(selected),
									},
								]
							: []),
						{
							id: "layer",
							label: "+ Add layer",
							onClick: () => setLayerModal("add"),
						},
						{
							id: "fixture",
							label: "+ Add fixture",
							onClick: () => setBrowserOpen(true),
						},
						{
							id: "multipatch",
							label: "+ Add multi-patch",
							disabled: !selected,
							onClick: () => void addMultipatch(),
						},
						{
							id: "delete",
							label: "Delete",
							active: deleteArmed,
							disabled: visible.length === 0,
							onClick: () => setDeleteArmed((armed) => !armed),
						},
					],
				]}
			/>
			<aside className="patch-layers">
				<h3>{layerModal === "select" ? "Select layer" : "Layers"}</h3>
				<Button
					className={activeLayer === "all" ? "active" : ""}
					onClick={() =>
						layerModal === "select" ? undefined : setActiveLayer("all")
					}
				>
					<b>All fixtures</b>
					<span>{all.length}</span>
				</Button>
				{layers.map((layer) => (
					<Button
						key={layer.id}
						className={activeLayer === layer.id ? "active" : ""}
						onClick={() =>
							layerModal === "select"
								? void selectLayer(layer.id)
								: setActiveLayer(layer.id)
						}
					>
						<b>{layer.name}</b>
						<span>
							{
								all.filter(
									(fixture) => (fixture.layer_id || "default") === layer.id,
								).length
							}
						</span>
					</Button>
				))}
			</aside>
			<section className="patch-table-wrap">
				<table className="patch-table">
					<thead>
						<tr>
							<th>Type</th>
							<th>Fixture ID</th>
							<th>Name</th>
							<th>Manufacturer</th>
							<th>Product / mode</th>
							<th>Patch</th>
							<th>MIB</th>
							<th>MIB Delay</th>
							<th>Highlight Look</th>
							<th>Location X/Y/Z</th>
							<th>Rotation X/Y/Z</th>
							<th>Layer</th>
						</tr>
					</thead>
					<tbody>
						{visible.map((fixture, index) => (
							<Fragment key={fixture.fixture_id}>
								<tr
									className={
										server.selectedFixtures.includes(fixture.fixture_id) ||
										fixture.logical_heads.some((head) =>
											server.selectedFixtures.includes(head.fixture_id),
										) ||
										selectedFixture === fixture.fixture_id
											? "selected"
											: ""
									}
									onClick={(event) => selectPatchFixture(fixture, event)}
								>
									<td className="patch-type-cell">
										<FixtureTypeIcon type={fixture.definition.device_type} />
									</td>
									<td>{fixtureDisplayId(fixture)}</td>
									<td>
										<Button
											className="patch-value"
											onClick={() => armEdit(fixture, "name")}
										>
											{fixture.name || fixture.definition.name}
										</Button>
									</td>
									<td>{fixture.definition.manufacturer}</td>
									<td>
										<Button
											className="patch-value"
											onClick={() => armEdit(fixture, "mode")}
										>
											{fixture.definition.model} · {fixture.definition.mode}
										</Button>
									</td>
									<td>
										{!isDmxPatchable(fixture.definition) ? (
											<span>Not patchable</span>
										) : definitionSplits(fixture.definition).length === 1 ? (
											<Button
												className="patch-address split-patch-summary"
												onClick={() => armEdit(fixture, "address")}
											>
												{formatFixturePatch(fixture)}
											</Button>
										) : (
											<div
												className="split-patch-targets"
												aria-label={`Fixture ${fixtureDisplayId(fixture)} split patches`}
											>
												{effectiveSplitPatches(
													fixture.definition,
													fixture.split_patches,
													fixture.universe,
													fixture.address,
												).map((patch) => (
													<Button
														key={patch.split}
														className="patch-address"
														active={
															selectedFixture === fixture.fixture_id &&
															editingSplit === patch.split
														}
														aria-label={`Split ${patch.split} patch ${patch.universe && patch.address ? `${patch.universe}.${patch.address}` : "unpatched"}`}
														onClick={(event) => {
															event.stopPropagation();
															selectSplitAddress(fixture, patch.split);
														}}
													>
														S{patch.split}{" "}
														{patch.universe && patch.address
															? `${patch.universe}.${patch.address}`
															: "—"}
													</Button>
												))}
											</div>
										)}
									</td>
									<td>
										{!isDmxPatchable(fixture.definition) ? (
											"—"
										) : (
											<Button
												className="patch-value"
												aria-label={`Move in Black ${fixtureDisplayId(fixture)}`}
												onClick={() => armEdit(fixture, "mib")}
											>
												{(fixture.move_in_black_enabled ?? true) ? "On" : "Off"}
											</Button>
										)}
									</td>
									<td>
										{!isDmxPatchable(fixture.definition) ? (
											"—"
										) : (
											<Button
												className="patch-value"
												aria-label={`MIB Delay ${fixtureDisplayId(fixture)}`}
												onClick={() => armEdit(fixture, "mib_delay")}
											>
												{(fixture.move_in_black_delay_millis ?? 0) / 1000} s
											</Button>
										)}
									</td>
									<td>
										{!isDmxPatchable(fixture.definition) ? (
											"—"
										) : (
											<Button
												className="patch-value"
												aria-label={`Highlight Look ${fixtureDisplayId(fixture)}`}
												onClick={() => armEdit(fixture, "highlight")}
											>
												{Object.keys(fixture.highlight_overrides ?? {}).length
													? `${Object.keys(fixture.highlight_overrides ?? {}).length} override${Object.keys(fixture.highlight_overrides ?? {}).length === 1 ? "" : "s"}`
													: "Profile default"}
											</Button>
										)}
									</td>
									<td className="patch-secondary">
										<Button
											className="patch-value"
											onClick={() => armEdit(fixture, "location")}
										>
											{(["x", "y", "z"] as const)
												.map((axis) =>
													((fixture.location?.[axis] ?? 0) / 1000).toFixed(3),
												)
												.join(" / ")}{" "}
											m
										</Button>
									</td>
									<td className="patch-secondary">
										<Button
											className="patch-value"
											onClick={() => armEdit(fixture, "rotation")}
										>
											{formatRotation(fixture.rotation)}
										</Button>
									</td>
									<td className="patch-secondary">
										<Button
											className="patch-value"
											onClick={() => {
												if (state.patchSetArmed) {
													setSelectedFixture(fixture.fixture_id);
													setLayerModal("select");
												}
											}}
										>
											{layers.find(
												(layer) => layer.id === (fixture.layer_id || "default"),
											)?.name ?? "Default"}
										</Button>
									</td>
								</tr>
								{(fixture.multipatch ?? []).map((instance, instanceIndex) => (
									<tr key={instance.id} className="multipatch-row">
										<td className="patch-tree-cell">
											<MultiPatchBranch
												last={
													instanceIndex ===
													(fixture.multipatch?.length ?? 0) - 1
												}
											/>
										</td>
										<td />
										<td className="multipatch-name">
											<span>multi-patch</span>
										</td>
										<td />
										<td />
										<td>
											{isDmxPatchable(fixture.definition) ? (
												<Button
													className="patch-address split-patch-summary"
													onClick={() =>
														beginMultipatchEdit(fixture, instance, "address")
													}
												>
													{formatInstancePatch(fixture.definition, instance)}
												</Button>
											) : (
												<span>Not patchable</span>
											)}
										</td>
										<td />
										<td />
										<td />
										<td className="patch-secondary">
											<Button
												className="patch-value"
												onClick={() =>
													beginMultipatchEdit(fixture, instance, "location")
												}
											>
												{(["x", "y", "z"] as const)
													.map((axis) =>
														(instance.location[axis] / 1000).toFixed(3),
													)
													.join(" / ")}{" "}
												m
											</Button>
										</td>
										<td className="patch-secondary">
											<Button
												className="patch-value"
												onClick={() =>
													beginMultipatchEdit(fixture, instance, "rotation")
												}
											>
												{formatRotation(instance.rotation)}
											</Button>
										</td>
										<td />
									</tr>
								))}
							</Fragment>
						))}
					</tbody>
				</table>
				{!visible.length && (
					<div className="patch-empty">No fixtures in this layer.</div>
				)}
				{stagePreviewOpen && (
					<div
						className="patch-stage-scroll-clearance"
						style={{ height: stagePreviewClearance }}
						aria-hidden="true"
					/>
				)}
			</section>
			{browserOpen && (
				<div className="stacked-modal-layer">
					<section className="nested-modal fixture-browser-modal">
						<ModalTitleBar
							title="Add fixture"
							search={
								<SearchBar
									value={query}
									onChange={setQuery}
									filters={[
										{ id: "type", label: "Fixture type", options: types },
									]}
									values={{ type: typeFilter }}
									onFilterChange={(_, value) => setTypeFilter(value)}
									placeholder="Search manufacturer, fixture, mode, or type"
								/>
							}
							closeLabel="Close Add fixture"
							onClose={() => setBrowserOpen(false)}
						/>
						<div className="fixture-picker-columns">
							<section>
								<h3>Manufacturer</h3>
								<Button
									className={!manufacturer ? "active" : ""}
									onClick={() => setManufacturer("")}
								>
									<span>All manufacturers</span>
								</Button>
								{manufacturers.map((name) => (
									<Button
										className={manufacturer === name ? "active" : ""}
										key={name}
										onClick={() => setManufacturer(name)}
									>
										<span>{name}</span>
									</Button>
								))}
							</section>
							<section>
								<h3>Fixture</h3>
								{families.map((item) => (
									<Button
										className={family?.key === item.key ? "active" : ""}
										key={item.key}
										onClick={() => chooseFamily(item.key)}
									>
										<span>{item.name}</span>
										<small>
											{item.deviceType} · {item.modes.length} modes
										</small>
									</Button>
								))}
							</section>
							<section className="fixture-mode-detail">
								{family && definition ? (
									<>
										<h3>
											{family.manufacturer} {family.name}
										</h3>
										<label>
											Mode
											<Select
												value={fixtureDefinitionKey(definition)}
												onChange={(event) =>
													setDefinitionKey(event.target.value)
												}
											>
												{family.modes.map((mode) => (
													<option
														value={fixtureDefinitionKey(mode)}
														key={fixtureDefinitionKey(mode)}
													>
														{mode.mode} ·{" "}
														{isDmxPatchable(mode)
															? `${mode.footprint}ch`
															: "No DMX"}
													</option>
												))}
											</Select>
										</label>
										<FixtureDetails definition={definition} />
										<Button className="primary" onClick={beginPlacement}>
											Add fixture
										</Button>
									</>
								) : (
									<p>Select a fixture.</p>
								)}
							</section>
						</div>
					</section>
				</div>
			)}
			{placementOpen && definition && (
				<div className="stacked-modal-layer">
					<section className="nested-modal fixture-placement-modal">
						<header>
							<h2>
								{isDmxPatchable(definition) ? "Patch" : "Add"} {family?.name}
							</h2>
							<Button onClick={requestPlacementClose}>Cancel</Button>
							<Button
								className="primary"
								disabled={
									busy ||
									(isDmxPatchable(definition)
										? parseFixtureNumber(draft.fixtureNumber) == null
										: parseVirtualFixtureNumber(draft.fixtureNumber) == null) ||
									(isDmxPatchable(definition) &&
										(definitionSplits(definition).length === 1
											? batchPatchError(
													batchPatches
														.slice(0, placementBatchCount(draft.count))
														.map(parsePatchAddress),
													definition.footprint,
													all,
												) != null
											: definitionSplits(definition).some(
													(split) =>
														Boolean(splitDrafts[split.number]?.trim()) &&
														!parsePatchAddress(splitDrafts[split.number]),
												)))
								}
								onClick={() => void addBatch()}
							>
								{busy ? "Adding…" : `Add ${draft.count || 1} fixtures`}
							</Button>
							<Button
								className="modal-close"
								aria-label="Close Add Fixture"
								onClick={requestPlacementClose}
							>
								×
							</Button>
						</header>
						<div className="placement-grid">
							<div className="placement-fields">
								<label>
									Mode
									<Select
										value={fixtureDefinitionKey(definition)}
										onChange={(event) => setDefinitionKey(event.target.value)}
									>
										{family?.modes.map((mode) => (
											<option
												value={fixtureDefinitionKey(mode)}
												key={fixtureDefinitionKey(mode)}
											>
												{mode.mode} ·{" "}
												{isDmxPatchable(mode)
													? `${mode.footprint}ch`
													: "No DMX"}
											</option>
										))}
									</Select>
								</label>
								<label>
									Fixture name
									<ConsoleTextField
										autoFocus
										value={draft.name}
										onChange={(name) => setDraft({ ...draft, name })}
									/>
								</label>
								<small>Trailing numbers increment automatically.</small>
								<label>
									Start fixture ID
									<ConsoleNumberField
										label="Start fixture ID"
										allowDecimal={!isDmxPatchable(definition)}
										value={draft.fixtureNumber}
										onChange={(fixtureNumber) =>
											setDraft({ ...draft, fixtureNumber })
										}
									/>
								</label>
								<small>Taken fixture IDs are skipped automatically.</small>
								<label>
									Count
									<ConsoleNumberField
										label="Count"
										value={draft.count}
										onChange={updatePlacementCount}
									/>
								</label>
								{!isDmxPatchable(definition) ? (
									<p>This Venue element is visual only and has no DMX patch.</p>
								) : definitionSplits(definition).length === 1 ? (
									<label>
										Address (universe.address)
										<ConsoleTextField
											value={draft.patch}
											onChange={updatePlacementPatch}
										/>
									</label>
								) : (
									<fieldset className="split-patch-fields">
										<legend>Independent split patches</legend>
										{definitionSplits(definition).map((split) => (
											<label key={split.number}>
												Split {split.number} · {split.footprint} slots
												<ConsoleTextField
													value={splitDrafts[split.number] ?? ""}
													onChange={(value) =>
														setSplitDrafts((current) => ({
															...current,
															[split.number]: value,
														}))
													}
												/>
												<small>Clear to leave this split unpatched.</small>
											</label>
										))}
									</fieldset>
								)}
								{status && <p className="patch-status">{status}</p>}
							</div>
							{isDmxPatchable(definition) && (
								<UniverseMap
									fixtures={all}
									universe={shownUniverse}
									proposed={shownAddress}
									footprint={
										definitionSplits(definition)[0]?.footprint ??
										definition.footprint
									}
									proposedLabel={`Fixture ${draft.fixtureNumber || "—"} · ${draft.name || definition.name}`}
									proposals={batchPatches
										.map((patch, index) => parsePatchAddress(patch))
										.filter(
											(patch): patch is { universe: number; address: number } =>
												Boolean(patch && patch.universe === shownUniverse),
										)
										.map((patch, index) => ({
											key: String(index),
											start: patch.address,
											footprint:
												definitionSplits(definition)[0]?.footprint ??
												definition.footprint,
											label: `Fixture ${(parseFixtureNumber(draft.fixtureNumber) ?? 1) + index} · ${incrementFixtureName(draft.name || definition.name, index)}`,
										}))}
									onAddress={(address) =>
										updateBatchPatch(0, shownUniverse, address)
									}
									onProposalAddress={(key, address) =>
										updateBatchPatch(Number(key), shownUniverse, address)
									}
									onUniverse={(universe) => {
										const address =
											firstFreeAddress(all, universe, definition.footprint) ??
											1;
										const patches = contiguousBatchPatches(
											universe,
											address,
											placementBatchCount(draft.count),
											definitionSplits(definition)[0]?.footprint ??
												definition.footprint,
										);
										setBatchPatches(patches);
										setDraft({ ...draft, patch: patches[0] });
										setSplitDrafts((current) => ({
											...current,
											[definitionSplits(definition)[0]?.number ?? 1]:
												patches[0],
										}));
									}}
								/>
							)}
						</div>
					</section>
				</div>
			)}
			{placementCloseConfirm && (
				<div className="stacked-modal-layer">
					<section
						className="nested-modal patch-small-modal"
						role="dialog"
						aria-modal="true"
						aria-labelledby="close-add-fixture-title"
					>
						<h3 id="close-add-fixture-title">Close Add Fixture?</h3>
						<p>
							Your changes in Add Fixture have not been applied. Do you really
							want to close?
						</p>
						<footer>
							<Button className="danger" onClick={closePlacement}>
								Yes, close
							</Button>
							<Button onClick={() => setPlacementCloseConfirm(false)}>
								Stay in Add Fixture
							</Button>
						</footer>
					</section>
				</div>
			)}
			{editCloseConfirm && (
				<div className="stacked-modal-layer">
					<section
						className="nested-modal patch-small-modal"
						role="dialog"
						aria-modal="true"
						aria-label="Discard fixture changes?"
					>
						<h3>Discard changes?</h3>
						<p>
							The changed{" "}
							{editCloseConfirm === "fixture" ? edit : multipatchEdit?.kind}{" "}
							values have not been saved.
						</p>
						<footer>
							<Button
								className="danger"
								onClick={() => {
									const target = editCloseConfirm;
									setEditCloseConfirm(null);
									if (target === "fixture") cancelEdit();
									else closeMultipatchEdit();
								}}
							>
								Discard changes
							</Button>
							<Button onClick={() => setEditCloseConfirm(null)}>
								Keep editing
							</Button>
						</footer>
					</section>
				</div>
			)}
			{deleteConfirm && (
				<div className="stacked-modal-layer">
					<section
						className="nested-modal patch-small-modal"
						role="alertdialog"
						aria-modal="true"
						aria-label={`Delete or unpatch ${deleteConfirm.name || deleteConfirm.definition.name}?`}
					>
						<h3>Delete or unpatch {fixtureDisplayId(deleteConfirm)}?</h3>
						<p>
							Delete removes{" "}
							<b>{deleteConfirm.name || deleteConfirm.definition.name}</b> from
							the show. Unpatch keeps the fixture line and clears its DMX
							addresses, including multi-patch addresses.
						</p>
						<footer>
							<Button
								className="danger"
								autoFocus
								onClick={() => void deleteFixture()}
							>
								Delete fixture
							</Button>
							<Button onClick={() => void unpatchFixtureFromDeleteConfirm()}>
								Unpatch fixture
							</Button>
							<Button onClick={() => setDeleteConfirm(null)}>Abort</Button>
						</footer>
					</section>
				</div>
			)}
			{multipatchEdit && multipatchEdit.kind !== "address" && (
				<div className="stacked-modal-layer">
					<section className="nested-modal patch-edit-modal">
						<ModalTitleBar
							title={`Set multi-patch ${multipatchEdit.kind}`}
							actions={
								<Button
									className="primary"
									onClick={() => void saveMultipatchEdit()}
								>
									Set
								</Button>
							}
							closeLabel={`Cancel multi-patch ${multipatchEdit.kind}`}
							onClose={requestMultipatchEditClose}
						/>
						{editError && (
							<p className="patch-status" role="alert">
								{editError}
							</p>
						)}
						<div className="vector-inputs">
							{(["x", "y", "z"] as const).map((axis) => (
								<NumberField
									key={axis}
									label={`${axis.toUpperCase()} ${multipatchEdit.kind === "location" ? "(m)" : ""}`}
									allowDecimal
									value={
										multipatchEdit.kind === "location"
											? vector[axis] / 1000
											: vector[axis]
									}
									onChange={(event) =>
										setVector({
											...vector,
											[axis]:
												multipatchEdit.kind === "location"
													? Math.round(Number(event.target.value) * 1000)
													: Number(event.target.value),
										})
									}
								/>
							))}
						</div>
					</section>
				</div>
			)}
			{multipatchEdit?.kind === "address" &&
				multipatchAddressFixture &&
				multipatchAddressInstance && (
					<div className="stacked-modal-layer fixture-address-layer">
						<FixtureAddressScreen
							fixture={multipatchAddressFixture}
							instance={multipatchAddressInstance}
							fixtures={all}
							initialSplit={null}
							singleValue={editText}
							splitValues={editSplitDrafts}
							error={editError}
							onSingleValue={setEditText}
							onSplitValues={setEditSplitDrafts}
							onCancel={closeMultipatchEdit}
							onConfirm={() => void saveMultipatchEdit()}
						/>
					</div>
				)}
			{layerModal === "add" && (
				<div className="stacked-modal-layer">
					<section className="nested-modal patch-small-modal">
						<h3>Add layer</h3>
						<TextInput
							clearable
							autoFocus
							aria-label="Layer name"
							value={layerName}
							onChange={(event) => setLayerName(event.target.value)}
							onKeyboardCommit={(value) => void createLayer(value)}
						/>
						<footer>
							<Button onClick={() => setLayerModal(null)}>Cancel</Button>
							<Button onClick={() => void createLayer()}>Add layer</Button>
						</footer>
					</section>
				</div>
			)}
			{edit && selected && edit !== "address" && (
				<div className="stacked-modal-layer">
					<section className="nested-modal patch-edit-modal">
						<ModalTitleBar
							title={`Set fixture ${edit === "mib" ? "MIB" : edit === "mib_delay" ? "MIB Delay" : edit === "highlight" ? "Highlight Look" : edit}`}
							actions={
								edit === "name" ? undefined : (
									<Button
										className="primary"
										onClick={() =>
											edit === "highlight" ? saveHighlightEdit() : saveEdit()
										}
									>
										Set
									</Button>
								)
							}
							closeLabel={`Cancel fixture ${edit}`}
							onClose={requestFixtureEditClose}
						/>
						{editError && (
							<p className="patch-status" role="alert">
								{editError}
							</p>
						)}
						{edit === "name" && (
							<TextInput
								clearable
								autoFocus
								aria-label="Fixture name"
								value={editText}
								onChange={(event) => setEditText(event.target.value)}
								onKeyboardCommit={saveEdit}
							/>
						)}{" "}
						{edit === "mib" && (
							<label>
								Move in Black
								<Select
									autoFocus
									aria-label="Move in Black value"
									value={editText}
									onChange={(event) => setEditText(event.target.value)}
								>
									<option value="true">Enabled</option>
									<option value="false">Disabled</option>
								</Select>
							</label>
						)}{" "}
						{edit === "mib_delay" && (
							<NumberField
								autoFocus
								label="MIB Delay (s)"
								min={0}
								step={0.1}
								allowDecimal
								value={editText}
								onChange={(event) => setEditText(event.target.value)}
							/>
						)}{" "}
						{edit === "highlight" && (
							<div className="fixture-highlight-look">
								<p>
									Blank values inherit the profile Highlight raw value.
									Overrides belong to this fixture and remain unchanged when its
									address changes.
								</p>
								{definitionModeChannels(selected.definition).map((channel) => (
									<NumberField
										key={channel.id}
										label={`${channel.attribute} highlight raw (profile ${channel.highlight_raw})`}
										min={0}
										max={maxRaw(channel.resolution)}
										value={highlightDrafts[channel.id] ?? ""}
										onChange={(event) =>
											setHighlightDrafts((current) => ({
												...current,
												[channel.id]: event.target.value,
											}))
										}
									/>
								))}
							</div>
						)}{" "}
						{(edit === "location" || edit === "rotation") && (
							<div className="vector-inputs">
								{(["x", "y", "z"] as const).map((axis) => (
									<NumberField
										key={axis}
										label={`${axis.toUpperCase()} ${edit === "location" ? "(m)" : ""}`}
										allowDecimal
										value={
											edit === "location" ? vector[axis] / 1000 : vector[axis]
										}
										onChange={(event) =>
											setVector({
												...vector,
												[axis]:
													edit === "location"
														? Math.round(Number(event.target.value) * 1000)
														: Number(event.target.value),
											})
										}
									/>
								))}
							</div>
						)}
						{edit === "mode" && selectedModeFamily && (
							<label>
								Product / mode
								<Select
									value={definitionKey}
									onChange={(event) => setDefinitionKey(event.target.value)}
								>
									{selectedModeFamily.modes.map((mode) => (
										<option
											value={fixtureDefinitionKey(mode)}
											key={fixtureDefinitionKey(mode)}
										>
											{mode.mode} · {mode.footprint}ch
										</option>
									))}
								</Select>
							</label>
						)}
					</section>
				</div>
			)}
			{pending && selected && (
				<div className="stacked-modal-layer">
					<section className="nested-modal conflict-modal">
						<h3>Patch conflict</h3>
						{editError && (
							<p className="patch-status" role="alert">
								{editError}
							</p>
						)}
						<p>
							The requested range overlaps{" "}
							{blockedBy
								.map(
									(fixture) =>
										`${fixture.name || fixture.definition.name} (${fixtureRange(fixture)?.universe}.${fixtureRange(fixture)?.start}–${fixtureRange(fixture)?.end})`,
								)
								.join(", ")}
							.
						</p>
						<footer>
							<Button
								onClick={() => {
									setPending(null);
									setBlockedBy([]);
								}}
							>
								Keep old patch / mode
							</Button>
							<Button onClick={() => void unpatchCurrentFixture()}>
								Unpatch current fixture
							</Button>
							<Button
								className="danger"
								onClick={() => void unpatchConflictsAndApply()}
							>
								Unpatch conflicts and apply
							</Button>
						</footer>
					</section>
				</div>
			)}
			{edit === "address" && selected && (
				<div className="stacked-modal-layer fixture-address-layer">
					<FixtureAddressScreen
						fixture={selected}
						fixtures={all}
						initialSplit={editingSplit}
						singleValue={editText}
						splitValues={editSplitDrafts}
						error={editError}
						onSingleValue={setEditText}
						onSplitValues={setEditSplitDrafts}
						onCancel={cancelEdit}
						onConfirm={() =>
							definitionSplits(selected.definition).length > 1
								? saveSplitEdit()
								: saveEdit()
						}
					/>
				</div>
			)}
		</div>
	);
}

export function fixtureDisplayId(
	fixture: Pick<PatchedFixture, "fixture_number" | "virtual_fixture_number">,
) {
	return fixture.virtual_fixture_number != null
		? `0.${fixture.virtual_fixture_number}`
		: (fixture.fixture_number ?? "—");
}
export function compareFixtureIds(a: PatchedFixture, b: PatchedFixture) {
	if (
		a.virtual_fixture_number != null &&
		b.virtual_fixture_number != null &&
		a.virtual_fixture_number !== b.virtual_fixture_number
	)
		return a.virtual_fixture_number - b.virtual_fixture_number;
	if (a.virtual_fixture_number != null) return -1;
	if (b.virtual_fixture_number != null) return 1;
	if (
		a.fixture_number != null &&
		b.fixture_number != null &&
		a.fixture_number !== b.fixture_number
	)
		return a.fixture_number - b.fixture_number;
	if (a.fixture_number != null) return -1;
	if (b.fixture_number != null) return 1;
	return a.fixture_id.localeCompare(b.fixture_id);
}
const MAX_FIXTURE_NUMBER = 4_294_967_295;
export function parseFixtureNumber(value: string): number | null {
	const number = Number(value);
	return Number.isInteger(number) && number >= 1 && number <= MAX_FIXTURE_NUMBER
		? number
		: null;
}
export function parseVirtualFixtureNumber(value: string): number | null {
	const match = /^0\.(\d+)$/.exec(value.trim());
	if (!match) return null;
	const number = Number(match[1]);
	return Number.isInteger(number) && number >= 1 && number <= MAX_FIXTURE_NUMBER
		? number
		: null;
}
export function nextAvailableFixtureNumber(
	start: number,
	used: ReadonlySet<number>,
): number | null {
	let number = start;
	while (number <= MAX_FIXTURE_NUMBER && used.has(number)) number++;
	return number <= MAX_FIXTURE_NUMBER ? number : null;
}
export function placementBatchCount(value: string) {
	return Math.max(1, Math.floor(Number(value) || 1));
}
export function contiguousBatchPatches(
	universe: number,
	address: number,
	count: number,
	footprint: number,
) {
	return Array.from(
		{ length: count },
		(_, index) => `${universe}.${address + index * footprint}`,
	);
}
export function resizeBatchPatches(
	current: string[],
	count: number,
	universe: number,
	address: number,
	footprint: number,
) {
	if (current.length >= count) return current.slice(0, count);
	const next = [...current];
	while (next.length < count) {
		const previous = parsePatchAddress(next.at(-1) ?? "") ?? {
			universe,
			address: address - footprint,
		};
		next.push(`${previous.universe}.${previous.address + footprint}`);
	}
	return next;
}
export function batchPatchError(
	patches: Array<{ universe: number; address: number } | null>,
	footprint: number,
	fixtures: PatchedFixture[],
) {
	if (!patches.length || patches.some((patch) => !patch))
		return "Choose a valid DMX address for every fixture in the batch.";
	const ranges = patches.map((patch, index) => ({
		index,
		universe: patch!.universe,
		start: patch!.address,
		end: patch!.address + footprint - 1,
	}));
	if (ranges.some((range) => range.start < 1 || range.end > 512))
		return "Every fixture in the batch must fit completely inside one 512-slot universe.";
	if (
		ranges.some(
			(range) =>
				conflicts(fixtures, range.universe, range.start, footprint).length,
		)
	)
		return "One or more fixture patches overlap an occupied DMX range.";
	for (let index = 0; index < ranges.length; index++)
		for (let other = index + 1; other < ranges.length; other++) {
			const left = ranges[index];
			const right = ranges[other];
			if (
				left.universe === right.universe &&
				left.start <= right.end &&
				right.start <= left.end
			)
				return `Fixture ${left.index + 1} overlaps fixture ${right.index + 1} in this batch.`;
		}
	return null;
}
export function definitionSplits(definition: FixtureDefinition) {
	const profile = definition.profile_snapshot;
	const mode =
		profile?.modes.find((candidate) => candidate.id === definition.mode_id) ??
		profile?.modes.find((candidate) => candidate.name === definition.mode) ??
		profile?.modes[0];
	return mode?.splits.length
		? mode.splits
		: [{ number: 1, footprint: definition.footprint }];
}
export function definitionModeChannels(
	definition: FixtureDefinition,
): FixtureChannel[] {
	const profile = definition.profile_snapshot;
	return (
		profile?.modes.find((candidate) => candidate.id === definition.mode_id)
			?.channels ??
		profile?.modes.find((candidate) => candidate.name === definition.mode)
			?.channels ??
		profile?.modes[0]?.channels ??
		[]
	);
}
export function effectiveSplitPatches(
	definition: FixtureDefinition,
	patches: SplitPatch[] | undefined,
	universe: number | null,
	address: number | null,
): SplitPatch[] {
	const configured = new Map(
		(patches ?? []).map((patch) => [patch.split, patch]),
	);
	return definitionSplits(definition).map(
		(split, index) =>
			configured.get(split.number) ?? {
				split: split.number,
				universe: index === 0 ? universe : null,
				address: index === 0 ? address : null,
			},
	);
}
export function reconcileSplitPatchOwner(
	currentDefinition: FixtureDefinition,
	nextDefinition: FixtureDefinition,
	patches: SplitPatch[] | undefined,
	universe: number | null,
	address: number | null,
): Pick<PatchedFixture, "split_patches" | "universe" | "address"> {
	const previous = new Map(
		effectiveSplitPatches(currentDefinition, patches, universe, address).map(
			(patch) => [patch.split, patch],
		),
	);
	const split_patches = definitionSplits(nextDefinition).map((split) => {
		const match = previous.get(split.number);
		return {
			split: split.number,
			universe: match?.universe ?? null,
			address: match?.address ?? null,
		};
	});
	const primary =
		split_patches.find((patch) => patch.split === 1) ?? split_patches[0];
	return {
		split_patches,
		universe: primary?.universe ?? null,
		address: primary?.address ?? null,
	};
}
export function reconcileModePatchChanges(
	fixture: PatchedFixture,
	definition: FixtureDefinition,
): Pick<
	PatchedFixture,
	"definition" | "split_patches" | "universe" | "address" | "multipatch"
> {
	if (!isDmxPatchable(definition)) {
		const clear = () => ({
			universe: null,
			address: null,
			split_patches: definitionSplits(definition).map((split) => ({
				split: split.number,
				universe: null,
				address: null,
			})),
		});
		return {
			definition,
			...clear(),
			multipatch: (fixture.multipatch ?? []).map((instance) => ({
				...instance,
				...clear(),
			})),
		};
	}
	const primary = reconcileSplitPatchOwner(
		fixture.definition,
		definition,
		fixture.split_patches,
		fixture.universe,
		fixture.address,
	);
	return {
		definition,
		...primary,
		multipatch: (fixture.multipatch ?? []).map((instance) => ({
			...instance,
			...reconcileSplitPatchOwner(
				fixture.definition,
				definition,
				instance.split_patches,
				instance.universe,
				instance.address,
			),
		})),
	};
}
export function unpatchFixtureChanges(
	fixture: PatchedFixture,
): Pick<
	PatchedFixture,
	"split_patches" | "universe" | "address" | "multipatch"
> {
	const clearOwner = (
		patches: SplitPatch[] | undefined,
		universe: number | null,
		address: number | null,
	) => ({
		universe: null,
		address: null,
		split_patches: effectiveSplitPatches(
			fixture.definition,
			patches,
			universe,
			address,
		).map((patch) => ({ split: patch.split, universe: null, address: null })),
	});
	return {
		...clearOwner(fixture.split_patches, fixture.universe, fixture.address),
		multipatch: (fixture.multipatch ?? []).map((instance) => ({
			...instance,
			...clearOwner(
				instance.split_patches,
				instance.universe,
				instance.address,
			),
		})),
	};
}
export function replaceSelectedSplitPatch(
	definition: FixtureDefinition,
	current: SplitPatch[] | undefined,
	universe: number | null,
	address: number | null,
	selectedSplit: number,
	patch: { universe: number; address: number } | null,
): Pick<PatchedFixture, "split_patches" | "universe" | "address"> {
	const split_patches = effectiveSplitPatches(
		definition,
		current,
		universe,
		address,
	).map((candidate) =>
		candidate.split === selectedSplit
			? {
					split: candidate.split,
					universe: patch?.universe ?? null,
					address: patch?.address ?? null,
				}
			: candidate,
	);
	const primary =
		split_patches.find((candidate) => candidate.split === 1) ??
		split_patches[0];
	return {
		split_patches,
		universe: primary?.universe ?? null,
		address: primary?.address ?? null,
	};
}
export function splitPatchSetError(
	definition: FixtureDefinition,
	patches: SplitPatch[],
) {
	const footprints = new Map(
		definitionSplits(definition).map((split) => [
			split.number,
			split.footprint,
		]),
	);
	const ranges = patches.flatMap((patch) => {
		if (patch.universe == null && patch.address == null) return [];
		if (patch.universe == null || patch.address == null)
			return [
				{
					split: patch.split,
					universe: patch.universe ?? 0,
					start: patch.address ?? 0,
					end: -1,
				},
			];
		const footprint = footprints.get(patch.split) ?? 0;
		return [
			{
				split: patch.split,
				universe: patch.universe,
				start: patch.address,
				end: patch.address + footprint - 1,
			},
		];
	});
	const invalid = ranges.find(
		(range) =>
			range.universe < 1 ||
			range.start < 1 ||
			range.end < range.start ||
			range.end > 512,
	);
	if (invalid)
		return `Split ${invalid.split} must fit completely inside one 512-slot universe.`;
	for (let index = 0; index < ranges.length; index++)
		for (let other = index + 1; other < ranges.length; other++) {
			const left = ranges[index];
			const right = ranges[other];
			if (
				left.universe === right.universe &&
				left.start <= right.end &&
				right.start <= left.end
			)
				return `Split ${left.split} overlaps split ${right.split}. Give each patched split its own address range.`;
		}
	return null;
}
export function formatFixturePatch(fixture: PatchedFixture) {
	const patches = effectiveSplitPatches(
		fixture.definition,
		fixture.split_patches,
		fixture.universe,
		fixture.address,
	);
	if (patches.length === 1)
		return patches[0].universe && patches[0].address
			? `${patches[0].universe}.${patches[0].address}`
			: "Unpatched";
	return patches
		.map(
			(patch) =>
				`S${patch.split} ${patch.universe && patch.address ? `${patch.universe}.${patch.address}` : "—"}`,
		)
		.join(" · ");
}
export function formatInstancePatch(
	definition: FixtureDefinition,
	instance: MultiPatchInstance,
) {
	const patches = effectiveSplitPatches(
		definition,
		instance.split_patches,
		instance.universe,
		instance.address,
	);
	if (patches.length === 1)
		return patches[0].universe && patches[0].address
			? `${patches[0].universe}.${patches[0].address}`
			: "Unpatched";
	return patches
		.map(
			(patch) =>
				`S${patch.split} ${patch.universe && patch.address ? `${patch.universe}.${patch.address}` : "—"}`,
		)
		.join(" · ");
}
function formatRotation(
	rotation: { x: number; y: number; z: number } | undefined,
) {
	return (["x", "y", "z"] as const)
		.map((axis) => `${Number((rotation?.[axis] ?? 0).toFixed(3))}°`)
		.join(" / ");
}

function FixtureTypeIcon({ type }: { type: string }) {
	const kind = fixtureTypeKind(type);
	return (
		<span
			className="fixture-type-icon"
			title={type || "other"}
			aria-label={`Type: ${type || "other"}`}
		>
			<svg viewBox="0 0 24 24" aria-hidden="true">
				{kind === "atmosphere" ? (
					<>
						<path d="M3 8c3-3 5 3 8 0s5 3 8 0M3 13c3-3 5 3 8 0s5 3 8 0M5 18c2-2 4 2 6 0s4 2 6 0" />
					</>
				) : kind === "moving" ? (
					<>
						<path d="M8 4h8l2 5-3 5H9L6 9zM12 14v4M7 20h10" />
						<path d="M16 6l5-2M17 9h5" />
					</>
				) : kind === "wash" ? (
					<>
						<path d="M8 4h8l2 5-3 5H9L6 9zM12 14v5M7 21h10" />
						<path d="M5 3 2 6M19 3l3 3" />
					</>
				) : kind === "profile" ? (
					<>
						<path d="M4 7h7l3 4-3 4H4zM14 9l7-3v10l-7-3" />
					</>
				) : kind === "strobe" ? (
					<path d="m13 2-8 12h6l-1 8 9-13h-6z" />
				) : kind === "media" ? (
					<>
						<rect x="3" y="4" width="18" height="14" rx="2" />
						<path d="m10 8 5 3-5 3zM8 21h8" />
					</>
				) : kind === "pixels" ? (
					<>
						<rect x="3" y="3" width="7" height="7" />
						<rect x="14" y="3" width="7" height="7" />
						<rect x="3" y="14" width="7" height="7" />
						<rect x="14" y="14" width="7" height="7" />
					</>
				) : kind === "dimmer" ? (
					<>
						<circle cx="12" cy="12" r="8" />
						<path d="M12 4a8 8 0 0 0 0 16zM12 1v2M12 21v2M1 12h2M21 12h2" />
					</>
				) : (
					<>
						<path d="M12 3 21 12 12 21 3 12z" />
						<circle cx="12" cy="12" r="2" />
					</>
				)}
			</svg>
		</span>
	);
}
function fixtureTypeKind(type: string) {
	const value = type.toLowerCase();
	if (/fog|haze|fan/.test(value)) return "atmosphere";
	if (/media|video/.test(value)) return "media";
	if (/pixel|strip|matrix/.test(value)) return "pixels";
	if (/strobe/.test(value)) return "strobe";
	if (/moving|mover|beam/.test(value)) return "moving";
	if (/wash/.test(value)) return "wash";
	if (/profile|spot/.test(value)) return "profile";
	if (/dimmer|relay/.test(value)) return "dimmer";
	return "other";
}
function MultiPatchBranch({ last }: { last: boolean }) {
	return (
		<span className="multipatch-branch" aria-hidden="true">
			<svg viewBox="0 0 28 42">
				<path d={last ? "M7 0v20q0 6 6 6h12" : "M7 0v42M7 20q0 6 6 6h12"} />
			</svg>
		</span>
	);
}

function FixtureDetails({ definition }: { definition: FixtureDefinition }) {
	return (
		<div className="fixture-details">
			<strong>
				{isDmxPatchable(definition)
					? `${definition.footprint} DMX channels`
					: "Visual only · no DMX patch"}
			</strong>
			<span>{definition.device_type}</span>
			<span>
				{definition.heads.length} head{definition.heads.length === 1 ? "" : "s"}
			</span>
			<span>Revision {definition.revision}</span>
			{definition.physical.width_millimetres && (
				<span>
					{definition.physical.width_millimetres} ×{" "}
					{definition.physical.height_millimetres ?? "?"} ×{" "}
					{definition.physical.depth_millimetres ?? "?"} mm
				</span>
			)}
		</div>
	);
}
const DMX_GRID_COLUMNS = 16;
export function dmxGridSegments(
	start: number,
	end: number,
	columns = DMX_GRID_COLUMNS,
) {
	const segments: Array<{ row: number; column: number; length: number }> = [];
	let address = start;
	while (address <= end) {
		const row = Math.floor((address - 1) / columns) + 1;
		const column = ((address - 1) % columns) + 1;
		const length = Math.min(end - address + 1, columns - column + 1);
		segments.push({ row, column, length });
		address += length;
	}
	return segments;
}
export function draggedDmxStart(
	address: number,
	offset: number,
	footprint: number,
) {
	return Math.max(
		1,
		Math.min(512 - Math.max(1, footprint) + 1, address - offset),
	);
}
export type UniverseMapProposal = {
	key: string;
	start: number;
	footprint: number;
	label: string;
};
export function UniverseMap({
	fixtures,
	universe,
	proposed,
	footprint,
	proposedLabel,
	proposals,
	onAddress,
	onProposalAddress,
	onUniverse,
}: {
	fixtures: PatchedFixture[];
	universe: number;
	proposed: number;
	footprint: number;
	proposedLabel: string;
	proposals?: UniverseMapProposal[];
	onAddress: (address: number) => void;
	onProposalAddress?: (key: string, address: number) => void;
	onUniverse: (universe: number) => void;
}) {
	const displayedProposals = proposals?.length
		? proposals
		: proposed > 0
			? [{ key: "primary", start: proposed, footprint, label: proposedLabel }]
			: [];
	const [selectedProposal, setSelectedProposal] = useState(
		displayedProposals[0]?.key ?? "primary",
	);
	const drag = useRef<{ key: string; offset: number } | null>(null);
	useEffect(() => {
		if (
			displayedProposals.length &&
			!displayedProposals.some(
				(candidate) => candidate.key === selectedProposal,
			)
		)
			setSelectedProposal(displayedProposals[0].key);
	}, [displayedProposals, selectedProposal]);
	const ranges = fixtures
		.flatMap((fixture) =>
			fixtureRanges(fixture).map((range, index) => ({ fixture, range, index })),
		)
		.filter((item) => item.range.universe === universe);
	const ownersByAddress = new Map<number, typeof ranges>();
	for (const item of ranges)
		for (let address = item.range.start; address <= item.range.end; address++)
			ownersByAddress.set(address, [
				...(ownersByAddress.get(address) ?? []),
				item,
			]);
	const proposalConflicts = new Map(
		displayedProposals.map((candidate) => {
			const end = candidate.start + candidate.footprint - 1;
			const overlapsBatch = displayedProposals.some(
				(other) =>
					other.key !== candidate.key &&
					candidate.start <= other.start + other.footprint - 1 &&
					other.start <= end,
			);
			return [
				candidate.key,
				end > 512 ||
					conflicts(fixtures, universe, candidate.start, candidate.footprint)
						.length > 0 ||
					overlapsBatch,
			];
		}),
	);
	const addressAtPointer = (event: React.PointerEvent) => {
		const target = document.elementFromPoint(
			event.clientX,
			event.clientY,
		) as HTMLElement | null;
		const cell = target?.closest("[data-dmx-address]") as HTMLElement | null;
		return cell ? Number(cell.dataset.dmxAddress) : null;
	};
	const moveProposal = (key: string, address: number) => {
		const candidate = displayedProposals.find((item) => item.key === key);
		if (!candidate) return;
		const next = draggedDmxStart(address, 0, candidate.footprint);
		if (onProposalAddress) onProposalAddress(key, next);
		else onAddress(next);
	};
	const finishDrag = () => {
		drag.current = null;
	};
	return (
		<section className="universe-visual">
			<header>
				<div>
					<h3>Universe {universe}</h3>
					<small>
						Tap an address or drag each blue fixture patch individually.
					</small>
				</div>
				<Select
					value={universe}
					onChange={(event) => onUniverse(Number(event.target.value))}
				>
					{Array.from({ length: 32 }, (_, index) => (
						<option key={index + 1}>{index + 1}</option>
					))}
				</Select>
			</header>
			<div className="dmx-address-grid-scroll">
				<div
					className="dmx-address-grid"
					role="grid"
					aria-label={`DMX universe ${universe}`}
					onPointerMove={(event) => {
						if (!drag.current) return;
						const address = addressAtPointer(event);
						const candidate = displayedProposals.find(
							(item) => item.key === drag.current?.key,
						);
						if (address != null && candidate) {
							const next = draggedDmxStart(
								address,
								drag.current.offset,
								candidate.footprint,
							);
							if (onProposalAddress) onProposalAddress(candidate.key, next);
							else onAddress(next);
						}
					}}
					onPointerUp={finishDrag}
					onPointerCancel={finishDrag}
				>
					{Array.from({ length: 512 }, (_, index) => {
						const address = index + 1;
						const owners = ownersByAddress.get(address) ?? [];
						const proposedHere = displayedProposals.filter(
							(candidate) =>
								address >= candidate.start &&
								address <= candidate.start + candidate.footprint - 1,
						);
						const hasConflict = proposedHere.some((candidate) =>
							proposalConflicts.get(candidate.key),
						);
						const ownerText = owners
							.map(
								({ fixture }) =>
									`Fixture ${fixture.fixture_number ?? "—"} ${fixture.name || fixture.definition.name}`,
							)
							.join(", ");
						const proposalText = proposedHere
							.map((candidate) => candidate.label)
							.join(", ");
						const stateText = [
							ownerText && `used by ${ownerText}`,
							proposalText &&
								(hasConflict
									? `conflicting proposed patch for ${proposalText}`
									: `proposed patch for ${proposalText}`),
						]
							.filter(Boolean)
							.join(", ");
						return (
							<Button
								key={address}
								className={`dmx-address-cell${owners.length ? " used" : ""}${proposedHere.length ? (hasConflict ? " proposed conflict" : " proposed") : ""}`}
								style={{
									gridRow: Math.floor(index / DMX_GRID_COLUMNS) + 1,
									gridColumn: (index % DMX_GRID_COLUMNS) + 1,
								}}
								data-dmx-address={address}
								aria-label={`DMX address ${address}${stateText ? `, ${stateText}` : ""}`}
								role="gridcell"
								onClick={() => {
									if (!proposedHere.length)
										moveProposal(selectedProposal, address);
								}}
								onPointerDown={(event) => {
									const candidate = proposedHere[0];
									if (!candidate) return;
									setSelectedProposal(candidate.key);
									drag.current = {
										key: candidate.key,
										offset: address - candidate.start,
									};
									event.currentTarget.setPointerCapture?.(event.pointerId);
									event.preventDefault();
								}}
							>
								{address}
							</Button>
						);
					})}
					{ranges.flatMap(({ fixture, range, index }) =>
						dmxGridSegments(range.start, range.end).map(
							(segment, segmentIndex) => (
								<div
									className="dmx-range-overlay used"
									key={`${fixture.fixture_id}-${index}-${segmentIndex}`}
									style={{
										gridRow: segment.row,
										gridColumn: `${segment.column} / span ${segment.length}`,
									}}
								>
									{segmentIndex === 0 && (
										<span>
											Fixture {fixture.fixture_number ?? "—"} ·{" "}
											{fixture.name || fixture.definition.name}
										</span>
									)}
								</div>
							),
						),
					)}
					{displayedProposals.flatMap((candidate) =>
						dmxGridSegments(
							candidate.start,
							Math.min(512, candidate.start + candidate.footprint - 1),
						).map((segment, segmentIndex) => (
							<div
								className={`dmx-range-overlay proposed${proposalConflicts.get(candidate.key) ? " conflict" : ""}${selectedProposal === candidate.key ? " selected" : ""}`}
								key={`${candidate.key}-${segmentIndex}`}
								style={{
									gridRow: segment.row,
									gridColumn: `${segment.column} / span ${segment.length}`,
								}}
							>
								{segmentIndex === 0 && <span>{candidate.label}</span>}
							</div>
						)),
					)}
				</div>
			</div>
		</section>
	);
}

function FixtureAddressScreen({
	fixture,
	instance,
	fixtures,
	initialSplit,
	singleValue,
	splitValues,
	error,
	onSingleValue,
	onSplitValues,
	onCancel,
	onConfirm,
}: {
	fixture: PatchedFixture;
	instance?: MultiPatchInstance;
	fixtures: PatchedFixture[];
	initialSplit: number | null;
	singleValue: string;
	splitValues: Record<number, string>;
	error: string;
	onSingleValue: (value: string) => void;
	onSplitValues: React.Dispatch<React.SetStateAction<Record<number, string>>>;
	onCancel: () => void;
	onConfirm: () => void;
}) {
	const splits = definitionSplits(fixture.definition);
	const [activeSplit, setActiveSplit] = useState(
		initialSplit ?? splits[0].number,
	);
	const split =
		splits.find((candidate) => candidate.number === activeSplit) ?? splits[0];
	const value =
		splits.length === 1 ? singleValue : (splitValues[split.number] ?? "");
	const parsed = parsePatchAddress(value);
	const setValue = (next: string) =>
		splits.length === 1
			? onSingleValue(next)
			: onSplitValues((current) => ({ ...current, [split.number]: next }));
	const append = (character: string) =>
		setValue(`${value}${character}`.replace(/^0+(?=\d)/, ""));
	const otherFixtures = fixtures.map((candidate) => {
		if (candidate.fixture_id !== fixture.fixture_id) return candidate;
		if (instance)
			return {
				...candidate,
				multipatch: (candidate.multipatch ?? []).filter(
					(item) => item.id !== instance.id,
				),
			};
		return {
			...candidate,
			universe: null,
			address: null,
			split_patches: definitionSplits(candidate.definition).map(
				(candidateSplit) => ({
					split: candidateSplit.number,
					universe: null,
					address: null,
				}),
			),
		};
	});
	const pendingPatches = splits.map((candidate) => {
		const raw =
			splits.length === 1
				? singleValue.trim()
				: (splitValues[candidate.number] ?? "").trim();
		const address = raw ? parsePatchAddress(raw) : null;
		return {
			split: candidate.number,
			raw,
			address,
			footprint: candidate.footprint,
		};
	});
	const syntaxError = pendingPatches.find(
		(candidate) => candidate.raw && !candidate.address,
	);
	const patchError = splitPatchSetError(
		fixture.definition,
		pendingPatches.map((candidate) => ({
			split: candidate.split,
			universe: candidate.address?.universe ?? null,
			address: candidate.address?.address ?? null,
		})),
	);
	const occupied = pendingPatches.find(
		(candidate) =>
			candidate.address &&
			conflicts(
				otherFixtures,
				candidate.address.universe,
				candidate.address.address,
				candidate.footprint,
			).length,
	);
	const invalidMessage = syntaxError
		? `Split ${syntaxError.split} must use universe.address.`
		: (patchError ??
			(occupied
				? `The complete Split ${occupied.split} footprint is unavailable at this address.`
				: ""));
	const invalid = Boolean(syntaxError || patchError);
	useEffect(() => {
		const closeOnEscape = (event: KeyboardEvent) => {
			if (event.key === "Escape") onCancel();
		};
		window.addEventListener("keydown", closeOnEscape, true);
		return () => window.removeEventListener("keydown", closeOnEscape, true);
	}, [onCancel]);
	const title = instance ? "Multi-patch Address" : "Fixture Address";
	const details = instance
		? `Fixture ${fixture.fixture_number ?? fixture.fixture_id} · ${instance.name}`
		: `Fixture ${fixture.fixture_number ?? fixture.fixture_id} · ${fixture.name || fixture.definition.name}`;
	return (
		<section
			className="nested-modal fixture-address-screen"
			role="dialog"
			aria-modal="true"
			aria-label={title}
		>
			<ModalTitleBar
				title={title}
				details={details}
				actions={
					<Button className="primary" disabled={invalid} onClick={onConfirm}>
						Set Address
					</Button>
				}
				closeLabel={`Cancel ${title}`}
				onClose={onCancel}
			/>
			<div className="fixture-address-summary">
				<span>
					Mode <b>{fixture.definition.mode}</b>
				</span>
				<span>
					Complete footprint{" "}
					<b>
						{splits.reduce(
							(total, candidate) => total + candidate.footprint,
							0,
						)}{" "}
						slots
					</b>
				</span>
				<span>
					Current{" "}
					<b>
						{instance
							? formatInstancePatch(fixture.definition, instance)
							: formatFixturePatch(fixture)}
					</b>
				</span>
				<span>
					Pending{" "}
					<b className={invalid ? "invalid" : ""}>{value || "Unpatched"}</b>
				</span>
			</div>
			{splits.length > 1 && (
				<nav aria-label="Address splits">
					{splits.map((candidate) => (
						<Button
							className={candidate.number === split.number ? "active" : ""}
							key={candidate.number}
							onClick={() => setActiveSplit(candidate.number)}
						>
							Split {candidate.number}
							<small>
								{candidate.footprint} slots ·{" "}
								{splitValues[candidate.number] || "Unpatched"}
							</small>
						</Button>
					))}
				</nav>
			)}
			<div className="fixture-address-content">
				<div className="fixture-address-entry">
					<label>
						Universe.address<strong>{value || "—"}</strong>
					</label>
					<div
						className="fixture-address-number-block"
						aria-label="Fixture address number block"
					>
						{["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", "⌫"].map(
							(key) => (
								<Button
									key={key}
									aria-label={
										key === "⌫"
											? "Backspace address"
											: key === "."
												? "Universe separator"
												: `Address ${key}`
									}
									onClick={() =>
										key === "⌫" ? setValue(value.slice(0, -1)) : append(key)
									}
								>
									{key}
								</Button>
							),
						)}
					</div>
					<Button className="unpatch" onClick={() => setValue("")}>
						Clear address · Unpatch
					</Button>
					{invalidMessage && <p role="alert">{invalidMessage}</p>}
					{error && <p role="alert">{error}</p>}
				</div>
				<UniverseMap
					fixtures={otherFixtures}
					universe={parsed?.universe ?? 1}
					proposed={parsed?.address ?? 0}
					footprint={split.footprint}
					proposedLabel={
						instance
							? `${instance.name} · Split ${split.number}`
							: `Fixture ${fixture.fixture_number ?? "—"} · Split ${split.number}`
					}
					onAddress={(address) =>
						setValue(`${parsed?.universe ?? 1}.${address}`)
					}
					onUniverse={(universe) =>
						setValue(
							`${universe}.${firstFreeAddress(otherFixtures, universe, split.footprint) ?? 1}`,
						)
					}
				/>
			</div>
		</section>
	);
}
