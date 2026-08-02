import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from "react";
import type { FixtureDefinition, PatchedFixture } from "../../api/types";
import type {
	PatchFixturePolicyAction,
	PatchFixtureProjection,
	PatchFixtureUpdateAction,
	PatchPlacement,
	PatchVectorSpread,
} from "./contracts";
import { PATCH_OBJECT_CHANGED_EVENT } from "./externalRepair";
import {
	createPatchDefinitionResolver,
	type PatchFixtureCandidate,
} from "./model";
import { PatchSession } from "./session";
import type { PatchStore, PatchStoreSnapshot } from "./store";
import type { PatchTransport } from "./transport";

export interface PatchedFixtureResult {
	fixtureId: string;
	selectionFixtureIds: readonly string[];
}

export interface SelectedPatchInstance {
	fixtureId: string;
	multipatchInstanceId: string | null;
}

export function reconcileSelectedPatchInstance(
	selection: SelectedPatchInstance | null,
	fixtures: readonly PatchedFixture[],
): SelectedPatchInstance | null {
	if (!selection) return null;
	const fixture = fixtures.find(
		(candidate) => candidate.fixture_id === selection.fixtureId,
	);
	if (!fixture) return null;
	if (
		selection.multipatchInstanceId &&
		!fixture.multipatch?.some(
			(instance) => instance.id === selection.multipatchInstanceId,
		)
	)
		return null;
	return selection;
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
	selectedPatchInstance: SelectedPatchInstance | null;
	selectPatchInstance(selection: SelectedPatchInstance | null): void;
	patchFixtures(
		candidates: readonly PatchFixtureCandidate[],
		placements?: readonly PatchPlacement[],
	): Promise<readonly PatchedFixtureResult[] | null>;
	updateFixture(
		fixtureId: string,
		changes: Partial<PatchedFixture>,
	): Promise<boolean>;
	spreadFixtureVector(spread: PatchVectorSpread): Promise<boolean>;
	updatePolicy(
		fixtureId: string,
		action: PatchFixturePolicyAction,
		changes: Partial<PatchedFixture>,
	): Promise<boolean>;
	updateFixtureIntent(
		fixtureId: string,
		multipatchInstanceId: string | null,
		action: PatchFixtureUpdateAction,
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
	const [selectedPatchInstance, selectPatchInstance] =
		useState<SelectedPatchInstance | null>(null);
	useEffect(() => () => session?.stop(), [session]);
	useEffect(() => selectPatchInstance(null), [showId]);
	useEffect(() => {
		selectPatchInstance((current) =>
			reconcileSelectedPatchInstance(current, snapshot.fixtures),
		);
	}, [snapshot.fixtures]);
	const value = useMemo<PatchContextValue>(
		() => ({
			...snapshot,
			selectedPatchInstance,
			selectPatchInstance,
			patchFixtures: async (candidates, placements = []) => {
				if (!session || snapshot.status !== "ready") return null;
				try {
					const outcome = await session.patchFixtures(
						candidates,
						[],
						placements,
					);
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
			spreadFixtureVector: async (spread) => {
				if (!session || snapshot.status !== "ready") return false;
				try {
					await session.spreadFixtureVector(spread);
					return true;
				} catch {
					return false;
				}
			},
			updatePolicy: async (fixtureId, action, changes) => {
				if (!session || snapshot.status !== "ready") return false;
				try {
					await session.updatePolicy(fixtureId, action, changes);
					return true;
				} catch {
					return false;
				}
			},
			updateFixtureIntent: async (fixtureId, multipatchInstanceId, action) => {
				if (!session || snapshot.status !== "ready") return false;
				try {
					await session.updateFixtureIntent(
						fixtureId,
						multipatchInstanceId,
						action,
					);
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
		[selectedPatchInstance, session, snapshot],
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

/**
 * The authoritative Patch store for scoped readers, or null outside a mounted Patch boundary.
 *
 * Readers must treat null as "no authority yet" and never fall back to bootstrap Patch data.
 */
export function usePatchStoreOrNull(): PatchStore | null {
	return useContext(PatchSessionContext)?.store ?? null;
}

/** Activates the exact Patch snapshot and stream only for a mounted Patch view. */
export function usePatchView(enabled = true): void {
	const session = useContext(PatchSessionContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activate();
	}, [enabled, session]);
	useEffect(() => {
		if (!session || !enabled) return;
		const repair = (event: Event) => {
			const detail = (event as CustomEvent<{ showId?: string }>).detail;
			if (detail?.showId === session.store.getSnapshot().showId)
				void session.refresh().catch(() => undefined);
		};
		window.addEventListener(PATCH_OBJECT_CHANGED_EVENT, repair);
		return () => window.removeEventListener(PATCH_OBJECT_CHANGED_EVENT, repair);
	}, [enabled, session]);
}

export type { PatchFixtureCandidate } from "./model";
export {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
	patchedFixtureCandidate,
} from "./model";
