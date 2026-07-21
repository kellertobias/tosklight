import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import type { ProgrammingInteractionStore } from "../programmingInteraction/store";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	CueTransferCapability,
	CueTransferConflictRepair,
	CueTransferScope,
	CueTransferTransport,
} from "./contracts";
import { CueTransferWriter } from "./writer";

interface CueTransferProviderProps {
	showId: string | null;
	deskId: string | null;
	userId: string | null;
	authorityKey: string;
	showStore: ShowObjectsStore;
	programmingStore: ProgrammingInteractionStore;
	transport: CueTransferTransport | null;
	repair: CueTransferConflictRepair | null;
	onError?: (error: Error | null) => void;
}

const CueTransferContext = createContext<CueTransferCapability | null>(null);

/** Action-only boundary; mounting it performs no fetch or event subscription. */
export function CueTransferProvider({
	children,
	showId,
	deskId,
	userId,
	authorityKey,
	showStore,
	programmingStore,
	transport,
	repair,
	onError,
}: PropsWithChildren<CueTransferProviderProps>) {
	const scope = useMemo<CueTransferScope | null>(
		() => (showId && deskId && userId ? { showId, deskId, userId } : null),
		[deskId, showId, userId],
	);
	const writer = useMemo(
		() =>
			scope && transport && repair
				? new CueTransferWriter({
						scope,
						showStore,
						programmingStore,
						transport,
						repair,
						onError,
					})
				: null,
		[
			authorityKey,
			onError,
			programmingStore,
			repair,
			scope,
			showStore,
			transport,
		],
	);
	useStrictModeSafeStop(writer);
	return (
		<CueTransferContext.Provider value={writer}>
			{children}
		</CueTransferContext.Provider>
	);
}

export function useCueTransfer() {
	return useContext(CueTransferContext);
}
