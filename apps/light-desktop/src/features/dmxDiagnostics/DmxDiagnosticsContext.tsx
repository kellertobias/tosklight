import { createContext, type PropsWithChildren, useContext } from "react";
import type {
	DmxSnapshot,
	OutputRoute,
	OutputRouteRangeIntent,
	VersionedObject,
} from "../../api/types";

/**
 * Scoped raw-DMX diagnostics: one-shot snapshot reads, per-slot overrides, and the
 * configured output routes, for the DMX monitor surfaces.
 */
export interface DmxDiagnostics {
	readDmx: () => Promise<DmxSnapshot>;
	setDmxOverride: (
		universe: number,
		address: number,
		value: number | null,
	) => Promise<void>;
	outputRoutes: VersionedObject<OutputRoute>[];
	saveOutputRoute: (
		id: string,
		route: OutputRoute,
		revision: number,
	) => Promise<boolean>;
	createOutputRouteRange: (range: OutputRouteRangeIntent) => Promise<boolean>;
	deleteOutputRoute: (id: string, revision: number) => Promise<boolean>;
}

const DmxDiagnosticsContext = createContext<DmxDiagnostics | null>(null);

export function DmxDiagnosticsProvider({
	children,
	diagnostics,
}: PropsWithChildren<{ diagnostics: DmxDiagnostics }>) {
	return (
		<DmxDiagnosticsContext.Provider value={diagnostics}>
			{children}
		</DmxDiagnosticsContext.Provider>
	);
}

/** DMX diagnostics, or null outside a mounted desk boundary. */
export function useDmxDiagnostics(): DmxDiagnostics | null {
	return useContext(DmxDiagnosticsContext);
}
