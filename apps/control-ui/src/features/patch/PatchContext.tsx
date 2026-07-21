import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useSyncExternalStore,
} from "react";
import type { FixtureDefinition, PatchedFixture } from "../../api/types";
import {
	createPatchDefinitionResolver,
	type PatchFixtureCandidate,
} from "./model";
import type { PatchFixtureProjection } from "./contracts";
import { PatchSession } from "./session";
import type { PatchStoreSnapshot } from "./store";
import type { PatchTransport } from "./transport";

export interface PatchedFixtureResult {
	fixtureId: string;
	selectionFixtureIds: readonly string[];
}

export function patchedFixtureResults(
	candidates: readonly PatchFixtureCandidate[],
	projections: readonly PatchFixtureProjection[],
): readonly PatchedFixtureResult[] {
	const byId = new Map(
		projections.map((fixture) => [fixture.fixtureId, fixture]),
	);
	return candidates.map((candidate) => {
		const fixtureId = candidate.fixture.fixture_id;
		const heads = byId.get(fixtureId)?.logicalHeads ?? [];
		return {
			fixtureId,
			selectionFixtureIds: heads.length
				? heads.map((head) => head.fixtureId)
				: [fixtureId],
		};
	});
}

export interface PatchContextValue extends PatchStoreSnapshot {
	patchFixtures(
		candidates: readonly PatchFixtureCandidate[],
	): Promise<readonly PatchedFixtureResult[] | null>;
	updateFixture(
		fixtureId: string,
		changes: Partial<PatchedFixture>,
	): Promise<boolean>;
	deleteFixture(fixtureId: string): Promise<boolean>;
}

interface PatchViewProviderProps {
	showId: string | null;
	initialFixtures: readonly PatchedFixture[];
	definitions: readonly FixtureDefinition[];
	transport: PatchTransport | null;
	onError?: (error: Error) => void;
}

const PatchContext = createContext<PatchContextValue | null>(null);
const PatchSessionContext = createContext<PatchSession | null>(null);
const noopSubscribe = () => () => undefined;

export function PatchViewProvider({
	children,
	showId,
	initialFixtures,
	definitions,
	transport,
	onError,
}: PropsWithChildren<PatchViewProviderProps>) {
	const resolver = useMemo(
		() => createPatchDefinitionResolver(definitions),
		[definitions],
	);
	const resolverRef = useRef(resolver);
	resolverRef.current = resolver;
	const errorRef = useRef(onError);
	errorRef.current = onError;
	const session = useMemo(() => {
		if (!showId || !transport) return null;
		return new PatchSession({
			showId,
			transport,
			initialFixtures,
			resolveDefinition: (...identity) => resolverRef.current(...identity),
			onError: (error) => errorRef.current?.(error),
		});
	}, [showId, transport]);
	const emptySnapshot = useMemo<PatchStoreSnapshot>(
		() => ({
			status: "loading",
			showId: showId ?? "",
			showRevision: null,
			patchRevision: null,
			cursor: null,
			fixtures: [],
			pendingFixtureIds: new Set(),
			error: null,
		}),
		[showId],
	);
	const snapshot = useSyncExternalStore(
		session?.store.subscribe ?? noopSubscribe,
		session?.store.getSnapshot ?? (() => emptySnapshot),
		session?.store.getSnapshot ?? (() => emptySnapshot),
	);
	useEffect(() => () => session?.stop(), [session]);
	const value = useMemo<PatchContextValue>(
		() => ({
			...snapshot,
			patchFixtures: async (candidates) => {
				if (!session || snapshot.status !== "ready") return null;
				try {
					const outcome = await session.patchFixtures(candidates);
					return patchedFixtureResults(candidates, outcome.fixtures);
				} catch {
					return null;
				}
			},
			updateFixture: async (fixtureId, changes) => {
				if (!session || snapshot.status !== "ready") return false;
				try {
					await session.updateFixture(fixtureId, changes);
					return true;
				} catch {
					return false;
				}
			},
			deleteFixture: async (fixtureId) => {
				if (!session || snapshot.status !== "ready") return false;
				try {
					await session.deleteFixture(fixtureId);
					return true;
				} catch {
					return false;
				}
			},
		}),
		[session, snapshot],
	);
	return (
		<PatchSessionContext.Provider value={session}>
			<PatchContext.Provider value={value}>{children}</PatchContext.Provider>
		</PatchSessionContext.Provider>
	);
}

export function usePatch(): PatchContextValue {
	const context = useContext(PatchContext);
	if (!context)
		throw new Error("usePatch must be used inside PatchViewProvider");
	return context;
}

export function useOptionalPatch(): PatchContextValue | null {
	return useContext(PatchContext);
}

/** Activates the exact Patch snapshot and stream only for a mounted Patch view. */
export function usePatchView(enabled = true): void {
	const session = useContext(PatchSessionContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activate();
	}, [enabled, session]);
}

export type { PatchFixtureCandidate } from "./model";
export {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
	patchedFixtureCandidate,
} from "./model";
