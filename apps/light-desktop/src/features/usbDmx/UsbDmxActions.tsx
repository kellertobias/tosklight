import { createContext, type PropsWithChildren, useContext } from "react";
import type {
	UsbDmxEndpoint,
	UsbDmxEndpointSnapshot,
} from "../../api/client/deskManagement";

export interface UsbDmxActions {
	load: () => Promise<UsbDmxEndpointSnapshot>;
	upsert: (
		revision: number,
		endpoint: UsbDmxEndpoint,
	) => Promise<UsbDmxEndpointSnapshot>;
	remove: (
		revision: number,
		endpointId: string,
	) => Promise<UsbDmxEndpointSnapshot>;
	resetMalformed: (revision: number) => Promise<UsbDmxEndpointSnapshot>;
}

const UsbDmxActionsContext = createContext<UsbDmxActions | null>(null);

export function UsbDmxActionsProvider({
	actions,
	children,
}: PropsWithChildren<{ actions: UsbDmxActions }>) {
	return (
		<UsbDmxActionsContext.Provider value={actions}>
			{children}
		</UsbDmxActionsContext.Provider>
	);
}

export function useUsbDmxActions(): UsbDmxActions | null {
	return useContext(UsbDmxActionsContext);
}
