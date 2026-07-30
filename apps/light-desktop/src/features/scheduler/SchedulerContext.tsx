import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
} from "react";
import type { SchedulerController } from "./contracts";

const SchedulerContext = createContext<SchedulerController | null>(null);

export function SchedulerProvider({
	children,
	controller,
}: PropsWithChildren<{ controller: SchedulerController }>) {
	return (
		<SchedulerContext.Provider value={controller}>
			{children}
		</SchedulerContext.Provider>
	);
}

export function useSchedulerController() {
	return useContext(SchedulerContext);
}

export function useSchedulerView(
	controller: SchedulerController | null,
	active: boolean,
) {
	useEffect(() => {
		if (!active || !controller?.activate) return;
		return controller.activate();
	}, [active, controller]);
}
