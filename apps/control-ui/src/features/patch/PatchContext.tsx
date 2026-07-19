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
import { PatchSession } from "./session";
import type { PatchStoreSnapshot } from "./store";
import type { PatchTransport } from "./transport";

export interface PatchContextValue extends PatchStoreSnapshot {
	patchFixtures(
		candidates: readonly PatchFixtureCandidate[],
	): Promise<string[] | null>;
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
			fixtures: initialFixtures,
			pendingFixtureIds: new Set(),
			error: null,
		}),
		[initialFixtures, showId],
	);
	const snapshot = useSyncExternalStore(
		session?.store.subscribe ?? noopSubscribe,
		session?.store.getSnapshot ?? (() => emptySnapshot),
		session?.store.getSnapshot ?? (() => emptySnapshot),
	);
	useEffect(() => {
		if (!session) return;
		void session.start().catch(() => undefined);
		return () => session.stop();
	}, [session]);
	const value = useMemo<PatchContextValue>(
		() => ({
			...snapshot,
			patchFixtures: async (candidates) => {
				if (!session) return null;
				try {
					await session.patchFixtures(candidates);
					return candidates.map((candidate) => candidate.fixture.fixture_id);
				} catch {
					return null;
				}
			},
			updateFixture: async (fixtureId, changes) => {
				if (!session) return false;
				try {
					await session.updateFixture(fixtureId, changes);
					return true;
				} catch {
					return false;
				}
			},
			deleteFixture: async (fixtureId) => {
				if (!session) return false;
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
		<PatchContext.Provider value={value}>{children}</PatchContext.Provider>
	);
}

export function usePatch(): PatchContextValue {
	const context = useContext(PatchContext);
	if (!context)
		throw new Error("usePatch must be used inside PatchViewProvider");
	return context;
}

export type { PatchFixtureCandidate } from "./model";
export {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
	patchedFixtureCandidate,
} from "./model";
