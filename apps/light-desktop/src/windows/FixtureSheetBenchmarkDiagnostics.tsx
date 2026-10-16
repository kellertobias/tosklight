import {
	Component,
	createContext,
	type ReactNode,
	useCallback,
	useContext,
	useEffect,
	useRef,
} from "react";
import { useDesktopBridge } from "../platform/desktop";
import type { PackagedStageBenchmarkConfig } from "../platform/desktop/types";

type Diagnostic = Record<string, unknown>;
type Report = (sample: Diagnostic) => void;
const BenchmarkProfile = createContext<string | null>(null);

function useBenchmarkReport(profile: string | undefined): Report {
	const desktop = useDesktopBridge();
	return useCallback(
		(sample: Diagnostic) => {
			if (!profile) return;
			const record = {
				schemaVersion: 1,
				measurementSurface: "packaged-tauri-webview-fixture-sheet",
				profile,
				recordedAt: new Date().toISOString(),
				...sample,
			};
			void desktop.appendPackagedStageBenchmarkSample(record).catch(() => {
				// The report channel may itself be the failed predicate. Keep a console fallback;
				// absence from both channels remains explicitly unknown in the runner.
				console.error(
					"Fixture Sheet benchmark diagnostic append failed",
					record,
				);
			});
		},
		[desktop, profile],
	);
}

class SheetRenderBoundary extends Component<
	{ children: ReactNode; report: Report },
	{ failed: boolean }
> {
	state = { failed: false };
	static getDerivedStateFromError() {
		return { failed: true };
	}
	componentDidCatch(error: unknown) {
		this.props.report({
			kind: "fixture-sheet-render-failed",
			errorName: error instanceof Error ? error.name : "Unknown",
			errorMessage: (error instanceof Error
				? error.message
				: String(error)
			).slice(0, 500),
		});
	}
	render() {
		return this.state.failed ? null : this.props.children;
	}
}

/** Only the packaged benchmark uses this boundary; operator windows keep their own behavior. */
export function FixtureSheetBenchmarkDiagnostics({
	profile,
	children,
}: {
	profile: string;
	children: ReactNode;
}) {
	const report = useBenchmarkReport(profile);
	useEffect(() => {
		report({ kind: "fixture-sheet-container-mounted" });
		return () => report({ kind: "fixture-sheet-container-unmounted" });
	}, [report]);
	return (
		<BenchmarkProfile.Provider value={profile}>
			<SheetRenderBoundary report={report}>{children}</SheetRenderBoundary>
		</BenchmarkProfile.Provider>
	);
}

/** Observe the predicates without satisfying or changing the separate readiness gate. */
export function useFixtureSheetReadinessDiagnostics({
	config,
	active,
	rows,
	activeValuesLoading,
	groupRuntimeLoading,
	ready = false,
}: {
	config: PackagedStageBenchmarkConfig | null | undefined;
	active: boolean;
	rows: Array<{ parentFixtureId: string }>;
	activeValuesLoading: boolean;
	groupRuntimeLoading: boolean;
	ready?: boolean;
}) {
	const parentProfile = useContext(BenchmarkProfile);
	const enabled = parentProfile !== null || Boolean(config?.fixtureSheet);
	const report = useBenchmarkReport(
		parentProfile ?? (config?.fixtureSheet ? config.profile : undefined),
	);
	const previous = useRef<string | null>(null);
	useEffect(() => {
		if (!enabled || ready) return;
		const sample = {
			kind: "fixture-sheet-readiness",
			active,
			configState:
				config === undefined
					? "loading"
					: config === null
						? "unavailable"
						: "loaded",
			fixtureSheetEnabled: config?.fixtureSheet ?? null,
			rowCount: rows.length,
			fixtureRecords: new Set(rows.map((row) => row.parentFixtureId)).size,
			expectedFixtureRecords: config?.expectedFixtureRecords ?? null,
			activeValuesLoading,
			groupRuntimeLoading,
		};
		const key = JSON.stringify(sample);
		if (key === previous.current) return;
		previous.current = key;
		report(sample);
	}, [
		config,
		active,
		rows,
		activeValuesLoading,
		groupRuntimeLoading,
		report,
		enabled,
		ready,
	]);
	return useCallback(
		() => report({ kind: "fixture-sheet-ready-append-failed" }),
		[report],
	);
}
