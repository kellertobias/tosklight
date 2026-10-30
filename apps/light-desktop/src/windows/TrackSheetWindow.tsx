import { Button, ErrorAlert, formatErrorDetails, SelectField, SwitchField, TextInput } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { useMemo, useState } from "react";
import type { CueList, VersionedObject } from "../api/types";
import { useAttributeRegistry } from "../features/deskSnapshot/DeskSnapshotState";
import { usePatchedFixturesView } from "../features/patch/PatchState";
import { cueListWriteBasis, useCueListTopologyWriter } from "../features/playbackTopology/useCueListTopologyWriter";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import { resolveGroupMembership } from "../features/showObjects/groupProjection";
import { useCueLists, usePortableGroups, usePresets } from "../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { TrackSheetGrid, type TrackSelection } from "./trackSheet/TrackSheetGrid";
import { editTrackValue, removeTrackValue, type TrackEditMode } from "./trackSheet/trackSheetEdits";
import {
	buildTrackSheet,
	filterTrackSheet,
	TRACK_FAMILIES,
	TRACK_FAMILY_LABELS,
	type TrackFamily,
	type TrackRow,
	type TrackSheetContext,
	trackCell,
} from "./trackSheet/trackSheetModel";
import { type PresetName, TRACK_STATE_LABELS, trackValueText } from "./trackSheet/trackSheetValues";
import "./trackSheet/trackSheet.css";
import type { WindowProps } from "./windowTypes";

const ALL = "__all__";
const NO_ATTRIBUTES: NonNullable<ReturnType<typeof useAttributeRegistry>> = [];
const SELECTED = "__selected__";

/** A Cuelist as a spreadsheet: where each value is set, tracks, is blocked, released or Cue only. */
export function TrackSheetWindow({ active = true, compact = false }: WindowProps) {
	useShowObjectView("cue_list", active);
	useShowObjectView("group", active);
	useShowObjectView("preset", active);
	const cueLists = useCueLists(active);
	const groups = usePortableGroups(active);
	const presetObjects = usePresets(active);
	const registry = useAttributeRegistry() ?? NO_ATTRIBUTES;
	const fixtures = usePatchedFixturesView(active);
	const selection = useProgrammingSelectionView(active);
	const selectionActions = useProgrammingSelectionActions(active);
	const saveCueList = useCueListTopologyWriter();
	const [cueListId, setCueListId] = useState("");
	const [fixtureFilter, setFixtureFilter] = useState(ALL);
	const [family, setFamily] = useState<TrackFamily | "all">("all");
	const [changesOnly, setChangesOnly] = useState(false);
	const [cell, setCell] = useState<TrackSelection | null>(null);
	const object = (cueLists.find((item) => item.body.id === cueListId) ?? cueLists[0]) as
		| VersionedObject<CueList>
		| undefined;
	const { groupMembers, context, presets, rowLabel } = useTrackSheetSources({ groups, presetObjects, registry, fixtures });
	const sheet = useMemo(() => (object ? buildTrackSheet(object.body, context) : null), [context, object]);
	const fixtureSet = useMemo(() => {
		if (fixtureFilter === ALL) return null;
		if (fixtureFilter === SELECTED) return new Set(selection?.selected ?? []);
		return new Set(groupMembers.get(fixtureFilter) ?? []);
	}, [fixtureFilter, groupMembers, selection]);
	const view = useMemo(
		() => (sheet ? filterTrackSheet(sheet, { fixtures: fixtureSet, family, changesOnly }) : null),
		[changesOnly, family, fixtureSet, sheet],
	);
	const filterOptions = [
		{ value: ALL, label: "All fixtures" },
		{ value: SELECTED, label: "Selected fixtures" },
		...groups.map((group) => ({ value: group.id, label: `Group ${group.body.name ?? group.id}` })),
	];
	return (
		<section className="track-sheet-window" aria-label="Track Sheet">
			{!compact && (
				<WindowHeader
					title="Track Sheet"
					info={{
						primary: object ? object.body.name : "No Cuelist",
						secondary: view ? `${view.columns.length} Cues · ${view.rows.length} values` : undefined,
					}}
				/>
			)}
			<div className="track-sheet-toolbar">
				<SelectField
					label="Cuelist"
					ariaLabel="Cuelist"
					value={object?.body.id ?? ""}
					options={cueLists.map((item) => ({
						value: item.body.id,
						label: `${item.body.pool_number ?? "—"} ${item.body.name}`,
					}))}
					disabled={!cueLists.length}
					onChange={(value) => {
						setCueListId(value);
						setCell(null);
					}}
				/>
				<SelectField label="Fixtures" ariaLabel="Fixtures" value={fixtureFilter} options={filterOptions} onChange={setFixtureFilter} />
				<SelectField
					label="Attributes"
					ariaLabel="Attributes"
					value={family}
					options={[
						{ value: "all", label: "All attributes" },
						...TRACK_FAMILIES.map((item) => ({ value: item, label: TRACK_FAMILY_LABELS[item] })),
					]}
					onChange={(value) => setFamily(value as TrackFamily | "all")}
				/>
				<SwitchField
					label="Changes only"
					checked={changesOnly}
					onChange={(event) => setChangesOnly(event.target.checked)}
				/>
			</div>
			<TrackSheetLegend />
			{!object || !sheet || !view ? (
				<p className="track-sheet-empty">Record a Cuelist to see its tracking here.</p>
			) : view.rows.length === 0 ? (
				<p className="track-sheet-empty">No stored values match these filters.</p>
			) : (
				<TrackSheetGrid
					rows={view.rows}
					columns={view.columns}
					cues={sheet.cues}
					presets={presets}
					rowLabel={rowLabel}
					selection={cell}
					onSelect={setCell}
				/>
			)}
			{object && sheet && cell && (
				<TrackCellEditor
					key={`${cell.rowKey}:${cell.cue}`}
					object={object}
					context={context}
					row={sheet.rows.find((row) => row.key === cell.rowKey) ?? null}
					cue={cell.cue}
					presets={presets}
					rowLabel={rowLabel}
					onSave={async (body) => {
						const saved = await saveCueList(cueListWriteBasis(object), body);
						if (!saved) throw new Error("The desk did not store the Cuelist. Check the desk connection and try again.");
					}}
					onSelectFixture={(fixtureId) =>
						void selectionActions?.replace({ resolvedFixtures: [fixtureId] })
					}
				/>
			)}
		</section>
	);
}

/** Group membership, attribute families, Preset names and row labels the sheet reads. */
function useTrackSheetSources({
	groups,
	presetObjects,
	registry,
	fixtures,
}: {
	groups: ReturnType<typeof usePortableGroups>;
	presetObjects: ReturnType<typeof usePresets>;
	registry: NonNullable<ReturnType<typeof useAttributeRegistry>>;
	fixtures: ReturnType<typeof usePatchedFixturesView>;
}) {
	const groupMembers = useMemo(() => resolveGroupMembership(groups), [groups]);
	const context = useMemo<TrackSheetContext>(() => {
		const families = new Map(registry.map((descriptor) => [descriptor.id, descriptor.family]));
		return {
			groupMembers,
			attributeFamily: (attribute) => families.get(attribute),
			fixtureOrder: fixtures.map((fixture) => fixture.fixture_id),
		};
	}, [fixtures, groupMembers, registry]);
	const presets = useMemo(
		() =>
			new Map<string, PresetName>(
				presetObjects.flatMap((preset) =>
					preset.body.instance_id
						? [[preset.body.instance_id, { name: preset.body.name, family: preset.body.family, number: preset.body.number }]]
						: [],
				),
			),
		[presetObjects],
	);
	const fixtureLabels = useMemo(
		() =>
			new Map(
				fixtures.map((fixture) => [
					fixture.fixture_id,
					`${fixture.fixture_number ?? fixture.virtual_fixture_number ?? "—"} ${fixture.name ?? ""}`.trim(),
				]),
			),
		[fixtures],
	);
	const attributeLabels = useMemo(
		() => new Map(registry.map((descriptor) => [descriptor.id, descriptor.label])),
		[registry],
	);
	const rowLabel = (row: TrackRow) => ({
		fixture: fixtureLabels.get(row.fixtureId) ?? `Fixture ${row.fixtureId.slice(0, 8)}`,
		attribute: attributeLabels.get(row.attribute) ?? row.attribute,
	});
	return { groupMembers, context, presets, rowLabel };
}

function TrackSheetLegend() {
	return (
		<ul className="track-sheet-legend" aria-label="Track sheet legend">
			{(["hard", "up", "down", "tracked", "block", "released", "restore"] as const).map((state) => (
				<li key={state} data-state={state}>
					<i aria-hidden="true" />
					{TRACK_STATE_LABELS[state]}
				</li>
			))}
			<li className="is-cue-only">
				<i aria-hidden="true" />
				Cue only
			</li>
		</ul>
	);
}

function TrackCellEditor({
	object,
	context,
	row,
	cue,
	presets,
	rowLabel,
	onSave,
	onSelectFixture,
}: {
	object: VersionedObject<CueList>;
	context: TrackSheetContext;
	row: TrackRow | null;
	cue: number;
	presets: ReadonlyMap<string, PresetName>;
	rowLabel: (row: TrackRow) => { fixture: string; attribute: string };
	onSave: (body: CueList) => Promise<void>;
	onSelectFixture: (fixtureId: string) => void;
}) {
	const state = row ? trackCell(row, cue) : null;
	const level = state?.value?.kind === "normalized" ? Math.round(state.value.value * 100) : null;
	const [draft, setDraft] = useState(level === null ? "" : String(level));
	const [busy, setBusy] = useState(false);
	const [message, setMessage] = useState("");
	const [error, setError] = useState("");
	if (!row || !state) return null;
	const label = rowLabel(row);
	const cueNumber = object.body.cues[cue]?.number ?? "";
	const percent = Number(draft);
	const editable = state.value === null || state.value.kind === "normalized";
	const draftValid = draft.trim() !== "" && Number.isFinite(percent) && percent >= 0 && percent <= 100;
	const storesOwn = object.body.cues[cue]?.changes.some(
		(change) =>
			!change.automatic_restore && change.fixture_id === row.fixtureId && change.attribute === row.attribute,
	);
	const run = async (description: string, build: () => CueList) => {
		setBusy(true);
		setError("");
		setMessage("");
		try {
			await onSave(build());
			setMessage(description);
		} catch (reason) {
			setError(formatErrorDetails(reason));
		} finally {
			setBusy(false);
		}
	};
	const write = (mode: TrackEditMode, description: string) =>
		run(description, () =>
			editTrackValue(object.body, context, {
				cue,
				fixtureId: row.fixtureId,
				attribute: row.attribute,
				// The desk stores levels as 32-bit floats; send exactly what it will store.
				value: { kind: "normalized", value: Math.fround(percent / 100) },
				mode,
			}),
		);
	return (
		<section className="track-sheet-editor" aria-label="Track sheet cell">
			<header>
				<strong>{`${label.fixture} · ${label.attribute} · Cue ${cueNumber}`}</strong>
				<span>
					{TRACK_STATE_LABELS[state.state]}
					{state.state !== "empty" && ` · ${trackValueText(state.value, state.preset, presets)}`}
					{state.sourceCue !== null && state.sourceCue !== cue &&
						` · from Cue ${object.body.cues[state.sourceCue]?.number}`}
					{state.groupId && " · stored on a Group"}
					{state.cueOnly && " · Cue only"}
				</span>
			</header>
			<div className="track-sheet-editor-actions">
				{editable ? (
					<TextInput
						aria-label="Value in percent"
						inputMode="decimal"
						value={draft}
						placeholder="0–100"
						disabled={busy}
						onChange={(event) => setDraft(event.target.value)}
					/>
				) : (
					<span className="track-sheet-editor-note">Change this value from the Programmer with Update.</span>
				)}
				{editable && (
					<>
						<Button variant="primary" disabled={busy || !draftValid} onClick={() => void write("cue", `Stored ${percent}% in Cue ${cueNumber}; it tracks on.`)}>
							Update Cue
						</Button>
						<Button disabled={busy || !draftValid} onClick={() => void write("cue_only", `Stored ${percent}% in Cue ${cueNumber} only.`)}>
							Cue Only
						</Button>
						<Button
							disabled={busy || !draftValid || state.sourceCue === null || state.sourceCue === cue}
							onClick={() => void write("source", `Updated the source Cue ${object.body.cues[state.sourceCue ?? 0]?.number}.`)}
						>
							Update Source Cue
						</Button>
					</>
				)}
				<Button
					disabled={busy || !storesOwn}
					onClick={() =>
						void run(`Removed the value stored in Cue ${cueNumber}.`, () =>
							removeTrackValue(object.body, { cue, fixtureId: row.fixtureId, attribute: row.attribute }),
						)
					}
				>
					Remove Value
				</Button>
				<Button disabled={busy} onClick={() => onSelectFixture(row.fixtureId)}>
					Select Fixture
				</Button>
			</div>
			{message && <p role="status">{message}</p>}
			{error && (
				<ErrorAlert as="p" role="alert">
					{error}
				</ErrorAlert>
			)}
		</section>
	);
}
