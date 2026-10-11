import { createContext, useContext, useState, type PropsWithChildren } from "react";
import { useScreensOptional } from "../../features/screens/ScreensContext";
import { PlaybackSectionSettingsModal } from "../modals/PlaybackSectionSettingsModal";

const Context = createContext<{ open(): void } | null>(null);
export const usePlaybackSectionSettings = () => useContext(Context);

/** Keeps settings alive while their local surface switches to its Playback grid. */
export function PlaybackSectionSettingsHost({ surfaceId, onShowGrid, children }: PropsWithChildren<{
	surfaceId: string;
	onShowGrid(): void;
}>) {
	const screens = useScreensOptional();
	const [open, setOpen] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const showGrid = async () => {
		setError(null);
		try {
			if (!screens) throw new Error("Playback surface is unavailable.");
			await screens.selectSurface(surfaceId);
			onShowGrid();
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	return (
		<Context.Provider value={{ open: () => { setError(null); setOpen(true); } }}>
			{children}
			{open && <>
				<PlaybackSectionSettingsModal surfaceId={surfaceId}
					onShowGrid={() => void showGrid()} preparationError={error}
					onClose={() => setOpen(false)} />
			</>}
		</Context.Provider>
	);
}
