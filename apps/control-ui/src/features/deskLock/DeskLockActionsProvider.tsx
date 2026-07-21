import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import type { DeskLockInput } from "../../api/client/configuration";
import type { DeskLockState } from "../../api/types";
import type { DeskLockStore } from "./store";

export interface DeskLockActions {
	configureDeskLock(input: DeskLockInput): Promise<boolean>;
	lockDesk(): Promise<void>;
	unlockDesk(pin?: string): Promise<boolean>;
}

interface DeskLockActionsProviderProps {
	store: DeskLockStore;
	configure(input: DeskLockInput): Promise<DeskLockState>;
	lock(): Promise<DeskLockState>;
	unlock(pin?: string): Promise<DeskLockState>;
	onError(message: string | null): void;
}

const DeskLockActionsContext = createContext<DeskLockActions | null>(null);

/** Mounting this action boundary performs no reads and no network work. */
export function DeskLockActionsProvider({
	children,
	store,
	configure,
	lock,
	unlock,
	onError,
}: PropsWithChildren<DeskLockActionsProviderProps>) {
	const actions = useMemo<DeskLockActions>(() => {
		const apply = async (next: Promise<DeskLockState>) => {
			store.install(await next);
			onError(null);
		};
		return {
			configureDeskLock: async (input) => {
				try {
					await apply(configure(input));
					return true;
				} catch (reason) {
					onError(asMessage(reason));
					return false;
				}
			},
			lockDesk: async () => {
				try {
					await apply(lock());
				} catch (reason) {
					onError(asMessage(reason));
				}
			},
			unlockDesk: async (pin) => {
				try {
					await apply(unlock(pin));
					return true;
				} catch (reason) {
					onError(asMessage(reason));
					return false;
				}
			},
		};
	}, [configure, lock, onError, store, unlock]);
	return (
		<DeskLockActionsContext.Provider value={actions}>
			{children}
		</DeskLockActionsContext.Provider>
	);
}

export function useDeskLockActions(): DeskLockActions | null {
	return useContext(DeskLockActionsContext);
}

function asMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
