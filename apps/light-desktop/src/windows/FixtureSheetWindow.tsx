import { FixtureSheetTableView as FixtureSheetTable } from "@tosklight/ui/tables";
import {
	type RowActivationModifiers,
	WindowHeader,
} from "@tosklight/ui/window-kit";
import {
	type ReactNode,
	useCallback,
	useDeferredValue,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { GroupStrip } from "../components/shared/GroupStrip";
import { SourceLegend } from "../components/shared/SourceLegend";
import { useHighlightSnapshot } from "../features/highlight/HighlightState";
import { useProgrammerPreloadLifecycleView } from "../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useProgrammerPreloadValuesView } from "../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { useProgrammerValuesView } from "../features/programmerValues/ProgrammerValuesView";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../features/programmingInteraction/ProgrammingInteractionView";
import { useDesktopBridge } from "../platform/desktop";
import { useApp } from "../state/AppContext";
import type { FixtureSheetColumn } from "../types";
import {
	DEFAULT_FIXTURE_SHEET_COLUMNS,
	FixtureSheetSettings,
} from "./FixtureSheetSettings";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
import { useFixtureSheetCuelistAuthority } from "./fixtureSheetCuelistAuthority";
import {
	useFixtureSheetRows,
	useFixtureSheetVisualizations,
} from "./fixtureSheetProjection";
import { createFixtureStepPresenter } from "./fixtureSheetStep";
import type { WindowProps } from "./windowTypes";

/**
 * Every fixture between two rows, in the order the sheet is showing them.
 *
 * The sheet's order is the operator's order: it follows whatever sort and filter is in force, so
 * a range is the run of rows they can see between the two they clicked, not a span of fixture
 * numbers that might not be next to each other on screen.
 */
/**
 * What clicking a row in the sheet does.
 *
 * The anchor is the last row an operator touched, held in a ref so it survives every re-render
 * the live values cause. Shift takes everything between it and this one, which is how a
 * run of lanterns down a bar gets selected in one gesture rather than a dozen. Anything else is
 * the ordinary single-fixture gesture, and either way this row becomes the anchor the next range
 * is measured from.
 */
function activateSheetRow(
	sheet: {
		rows: readonly { parentFixtureId: string }[];
		selectionActions: ReturnType<typeof useProgrammingSelectionActions>;
		selectionAnchor: { current: string | null };
	},
	fixtureId: string,
	modifiers: RowActivationModifiers,
) {
	const anchor = sheet.selectionAnchor.current;
	const range =
		modifiers.range && anchor
			? fixtureRange(sheet.rows, anchor, fixtureId)
			: null;
	sheet.selectionAnchor.current = fixtureId;
	if (range) {
		void sheet.selectionActions?.replace({ resolvedFixtures: range });
		return;
	}
	void sheet.selectionActions?.gesture({
		source: { type: "fixture", fixtureId },
		resolvedFixtures: [fixtureId],
	});
}

export function fixtureRange(
	rows: readonly { parentFixtureId: string }[],
	anchorFixtureId: string,
	fixtureId: string,
): string[] | null {
	const ordered = [...new Set(rows.map((row) => row.parentFixtureId))];
	const from = ordered.indexOf(anchorFixtureId);
	const to = ordered.indexOf(fixtureId);
	// An anchor that has been filtered away leaves nothing to measure from.
	if (from < 0 || to < 0) return null;
	return ordered.slice(Math.min(from, to), Math.max(from, to) + 1);
}

export function FixtureSheetWindow({
	active = true,
	compact,
	viewOnly = false,
	showGroupShortcuts,
	fixtureSheetIncludedHeads,
	fixtureSheetOrder,
	fixtureSheetActiveOnly,
	fixtureSheetCompactMode,
	fixtureSheetCueListId,
	fixtureSheetColumns: forcedColumns,
	fixtureSheetShowType,
}: WindowProps) {
	const highlight = useHighlightSnapshot();
	const interactionActive = active && !viewOnly;
	const selection = useProgrammingSelectionView(interactionActive);
	const selectionActions = useProgrammingSelectionActions(interactionActive);
	const preload = useProgrammerPreloadLifecycleView(active);
	const { state } = useApp();
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [activeRow, setActiveRow] = useState(0);
	const [visibleFixtureIds, onVisibleFixtureIdsChange] = useVisibleFixtureIds();
	const groupsVisible = compact
		? Boolean(showGroupShortcuts)
		: state.fixtureGroupsVisible;
	const fixtureOrder = compact
		? (fixtureSheetOrder ?? "fixture-id")
		: state.fixtureSheetOrder;
	const activeOnly = compact
		? (fixtureSheetActiveOnly ?? false)
		: state.fixtureSheetActiveOnly;
	const compactMode = compact
		? (fixtureSheetCompactMode ?? "off")
		: state.fixtureSheetCompactMode;
	const cuelistFilter = useFixtureSheetCuelistAuthority({
		enabled: active && (!compact || fixtureSheetCueListId != null),
		savedCueListId: compact
			? (fixtureSheetCueListId ?? "")
			: state.fixtureSheetCueListId,
	});
	const cueListId = cuelistFilter.selectedCueListId;
	const visibleColumnIds = compact
		? (forcedColumns ?? DEFAULT_FIXTURE_SHEET_COLUMNS)
		: state.fixtureSheetColumns;
	const showType = compact
		? (fixtureSheetShowType ?? true)
		: state.fixtureSheetShowType;
	const includedHeads = compact
		? (fixtureSheetIncludedHeads ?? "all")
		: state.fixtureSheetIncludedHeads;
	const { visualization, preloadVisualization } = useFixtureSheetVisualizations(
		preload.armed || preload.active,
		active,
		visibleFixtureIds,
	);
	const programmerValues = useProgrammerValuesView(active);
	const preloadProgrammerValues = useProgrammerPreloadValuesView(
		active && (preload.armed || preload.active),
	);
	const deferredVisualization = useDeferredValue(visualization);
	const deferredPreloadVisualization = useDeferredValue(preloadVisualization);
	const deferredProgrammerValues = useDeferredValue(programmerValues);
	const deferredPreloadProgrammerValues = useDeferredValue(
		preloadProgrammerValues,
	);
	const { rows, activeValuesLoading, groupRuntimeLoading } =
		useFixtureSheetRows({
			visualization: deferredVisualization,
			preloadVisualization: deferredPreloadVisualization,
			programmerValues: deferredProgrammerValues,
			preloadProgrammerValues: deferredPreloadProgrammerValues,
			fixtureOrder,
			activeOnly,
			selectedCueList: cuelistFilter.selectedCueList,
			includedHeads,
			highlight,
			active,
		});
	useFixtureSheetBenchmarkReady({
		active,
		rows,
		activeValuesLoading,
		groupRuntimeLoading,
	});
	const presentStep = useMemo(
		() => createFixtureStepPresenter(highlight),
		[highlight],
	);
	const columns = useMemo(
		() =>
			fixtureSheetColumns(showType, presentStep, compactMode).filter((column) =>
				visibleColumnIds.includes(column.id as FixtureSheetColumn),
			),
		[compactMode, presentStep, showType, visibleColumnIds],
	);
	const selectedFixtureIds = useMemo(
		() => new Set(selection?.selected ?? []),
		[selection?.selected],
	);
	const selectionActionStatus = resolveSelectionActionStatus(selectionActions);
	const selectionAnchor = useRef<string | null>(null);
	const activateRow = (fixtureId: string, modifiers: RowActivationModifiers) =>
		activateSheetRow(
			{ rows, selectionActions, selectionAnchor },
			fixtureId,
			modifiers,
		);

	return (
		<FixtureSheetWindowView
			compact={compact}
			compactMode={compactMode}
			selectionCount={selection ? selection.selected.length : null}
			selectionActionStatus={selectionActionStatus}
			info={<SourceLegend />}
			activeValuesLoading={activeValuesLoading}
			groupRuntimeLoading={groupRuntimeLoading}
			onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())}
			table={
				<FixtureSheetTable
					activeRow={activeRow}
					columns={columns}
					onActivate={viewOnly ? undefined : activateRow}
					onActiveRowChange={viewOnly ? undefined : setActiveRow}
					onVisibleFixtureIdsChange={onVisibleFixtureIdsChange}
					presentStep={presentStep}
					rows={rows}
					rowHeight={compactMode === "off" ? 43 : 32}
					selectedFixtureIds={selectedFixtureIds}
				/>
			}
			groups={
				groupsVisible ? (
					<GroupStrip active={active} viewOnly={viewOnly} />
				) : null
			}
			settings={
				settingsAnchor ? (
					<FixtureSheetSettings
						activeOnly={activeOnly}
						compactMode={compactMode}
						anchor={settingsAnchor}
						cueLists={cuelistFilter.cueLists}
						cueListId={cueListId}
						fixtureOrder={fixtureOrder}
						groupsVisible={groupsVisible}
						includedHeads={includedHeads}
						onClose={() => setSettingsAnchor(null)}
					/>
				) : null
			}
		/>
	);
}

export function useFixtureSheetBenchmarkReady({
	active,
	rows,
	activeValuesLoading,
	groupRuntimeLoading,
}: {
	active: boolean;
	rows: Array<{ parentFixtureId: string }>;
	activeValuesLoading: boolean;
	groupRuntimeLoading: boolean;
}) {
	const desktop = useDesktopBridge();
	const reported = useRef(false);
	const [ready, setReady] = useState(false);
	const [config, setConfig] = useState<
		Awaited<ReturnType<typeof desktop.packagedStageBenchmarkConfig>> | undefined
	>();
	useEffect(() => {
		if (typeof desktop.packagedStageBenchmarkConfig !== "function") {
			setConfig(null);
			return;
		}
		let cancelled = false;
		void desktop
			.packagedStageBenchmarkConfig()
			.then((value) => {
				if (!cancelled) setConfig(value);
			})
			.catch(() => {
				if (!cancelled) setConfig(null);
			});
		return () => {
			cancelled = true;
		};
	}, [desktop]);
	useEffect(() => {
		if (
			reported.current ||
			!config?.fixtureSheet ||
			!active ||
			activeValuesLoading ||
			groupRuntimeLoading ||
			rows.length === 0
		)
			return;
		const fixtureRecords = new Set(rows.map((row) => row.parentFixtureId)).size;
		if (
			config.expectedFixtureRecords != null &&
			fixtureRecords !== config.expectedFixtureRecords
		)
			return;
		reported.current = true;
		void desktop
			.appendPackagedStageBenchmarkSample({
				schemaVersion: 1,
				kind: "fixture-sheet-ready",
				measurementSurface: "packaged-tauri-webview-fixture-sheet",
				profile: config.profile,
				fixtureRecords,
				rowCount: rows.length,
				recordedAt: new Date().toISOString(),
			})
			.catch(() => {
				reported.current = false;
				setReady(false);
			});
		setReady(true);
	}, [active, activeValuesLoading, config, desktop, groupRuntimeLoading, rows]);
	// Each beat reads the rows of the moment. Depending on `rows` instead would tie the
	// interval's life to the data's identity: live values give the Sheet a new rows array
	// every couple of seconds, which tore the interval down before it could fire and made a
	// perfectly healthy Sheet look like it had stopped.
	const latestRows = useRef(rows);
	latestRows.current = rows;
	useEffect(() => {
		if (
			!ready ||
			!config?.fixtureSheet ||
			!active ||
			activeValuesLoading ||
			groupRuntimeLoading
		)
			return;
		const heartbeat = window.setInterval(() => {
			const current = latestRows.current;
			const fixtureRecords = new Set(
				current.map((row) => row.parentFixtureId),
			).size;
			// A row count that has not settled yet skips this beat rather than ending the
			// heartbeat, so a transient mismatch cannot silence the Sheet for good.
			if (
				config.expectedFixtureRecords != null &&
				fixtureRecords !== config.expectedFixtureRecords
			)
				return;
			void desktop
				.appendPackagedStageBenchmarkSample({
					schemaVersion: 1,
					kind: "fixture-sheet-heartbeat",
					measurementSurface: "packaged-tauri-webview-fixture-sheet",
					profile: config.profile,
					fixtureRecords,
					rowCount: current.length,
					recordedAt: new Date().toISOString(),
				})
				.catch(() => undefined);
		}, 1_000);
		return () => window.clearInterval(heartbeat);
	}, [active, activeValuesLoading, config, desktop, groupRuntimeLoading, ready]);
}

function useVisibleFixtureIds() {
	const [fixtureIds, setFixtureIds] = useState<readonly string[]>([]);
	const onChange = useCallback((next: readonly string[]) => {
		setFixtureIds((current) =>
			current.length === next.length &&
			current.every((fixtureId, index) => fixtureId === next[index])
				? current
				: [...next],
		);
	}, []);
	return [fixtureIds, onChange] as const;
}

function resolveSelectionActionStatus(
	actions: ReturnType<typeof useProgrammingSelectionActions>,
) {
	if (typeof actions?.status === "function") return actions.status();
	return actions ? ("ready" as const) : ("loading" as const);
}

export function FixtureSheetWindowView({
	compact,
	compactMode = "off",
	selectionCount,
	selectionActionStatus,
	info,
	activeValuesLoading,
	groupRuntimeLoading,
	onSettings,
	table,
	groups,
	settings,
}: {
	compact?: boolean;
	compactMode?: import("../types").FixtureSheetCompactMode;
	selectionCount: number | null;
	selectionActionStatus?: "ready" | "loading" | "scope-mismatch" | "stopped";
	info?: ReactNode;
	activeValuesLoading?: boolean;
	groupRuntimeLoading?: boolean;
	onSettings?: (anchor: HTMLElement) => void;
	table: ReactNode;
	groups?: ReactNode;
	settings?: ReactNode;
}) {
	return (
		<div
			className={`fixture-window fixture-sheet-mode-${compactMode}`}
			data-fixture-sheet-compact-mode={compactMode}
			data-selection-action-status={selectionActionStatus}
		>
			{!compact && (
				<WindowHeader
					title="Fixture Sheet"
					info={{
						primary:
							selectionCount == null
								? "Selection loading…"
								: `${selectionCount} selected`,
						secondary: info,
					}}
					settings
					onSettings={onSettings}
				/>
			)}
			{activeValuesLoading && (
				<p className="fixture-sheet-loading" role="status">
					Programmer values loading…
				</p>
			)}
			{groupRuntimeLoading && (
				<p className="fixture-sheet-loading" role="status">
					Group runtime loading…
				</p>
			)}
			{table}
			{groups}
			{settings}
		</div>
	);
}
