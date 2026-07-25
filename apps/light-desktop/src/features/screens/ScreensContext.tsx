import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import { useStableCallback } from "../shared/useStableCallback";
import type { ScreensContextValue } from "./types";

const ScreensContext = createContext<ScreensContextValue | null>(null);

export function ScreensProvider({
	source,
	children,
}: PropsWithChildren<{ source: ScreensContextValue }>) {
	const saveScreen = useStableCallback(source.saveScreen);
	const deleteScreen = useStableCallback(source.deleteScreen);
	const setScreenPage = useStableCallback(source.setScreenPage);
	const updateControlDesk = useStableCallback(source.updateControlDesk);
	const selectControlDesk = useStableCallback(source.selectControlDesk);
	const removeClient = useStableCallback(source.removeClient);
	const value = useMemo(
		() => ({
			screens: source.screens,
			bootstrap: source.bootstrap,
			session: source.session,
			saveScreen,
			deleteScreen,
			setScreenPage,
			updateControlDesk,
			selectControlDesk,
			removeClient,
		}),
		[
			source.screens,
			source.bootstrap,
			source.session,
			saveScreen,
			deleteScreen,
			setScreenPage,
			updateControlDesk,
			selectControlDesk,
			removeClient,
		],
	);
	return (
		<ScreensContext.Provider value={value}>{children}</ScreensContext.Provider>
	);
}

export function useScreens() {
	const context = useContext(ScreensContext);
	if (!context)
		throw new Error("useScreens must be used inside ScreensProvider");
	return context;
}
