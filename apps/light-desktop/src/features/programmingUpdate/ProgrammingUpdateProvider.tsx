import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	ProgrammingUpdateCapability,
	ProgrammingUpdateScope,
	ProgrammingUpdateTransport,
} from "./contracts";
import { ProgrammingUpdateWriter } from "./writer";

interface ProgrammingUpdateProviderProps {
	showId: string | null;
	deskId: string | null;
	userId: string | null;
	initialShowRevision: number | null;
	authorityKey: string;
	store: ShowObjectsStore;
	transport: ProgrammingUpdateTransport | null;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
}

const ProgrammingUpdateContext =
	createContext<ProgrammingUpdateCapability | null>(null);

/** Action-only scoped boundary; construction performs no fetch or subscription. */
export function ProgrammingUpdateProvider({
	children,
	showId,
	deskId,
	userId,
	initialShowRevision,
	authorityKey,
	store,
	transport,
	loadObject,
}: PropsWithChildren<ProgrammingUpdateProviderProps>) {
	const scope = useMemo<ProgrammingUpdateScope | null>(
		() =>
			showId && deskId && userId
				? { showId, deskId, userId, initialShowRevision }
				: null,
		[deskId, initialShowRevision, showId, userId],
	);
	const scopeKey = scope
		? [authorityKey, scope.showId, scope.deskId, scope.userId].join("|")
		: authorityKey;
	const writer = useMemo(
		() =>
			scope && transport
				? new ProgrammingUpdateWriter({
						scopeKey,
						scope,
						store,
						transport,
						loadObject,
					})
				: null,
		[loadObject, scope, scopeKey, store, transport],
	);
	useStrictModeSafeStop(writer);
	return (
		<ProgrammingUpdateContext.Provider value={writer}>
			{children}
		</ProgrammingUpdateContext.Provider>
	);
}

export function useProgrammingUpdate() {
	return useContext(ProgrammingUpdateContext);
}
