import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect, useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { PlaybackSectionSettingsHost, usePlaybackSectionSettings } from "./PlaybackSectionSettingsHost";
import { PlaybackSoloEditorProvider, usePlaybackSoloEditorRegistry, type PlaybackSoloEditor } from "../../features/playbackSolo/PlaybackSoloEditorContext";
const mocks = vi.hoisted(() => ({ select: vi.fn(), begin: vi.fn(), regions: [] }));
vi.mock("../../features/screens/ScreensContext", () => {
	const useScreens = () => ({ selectSurface: mocks.select, surfaces: {
		playback_surfaces: { main: { name: "Main", page_source: { type: "own", page: 2 } } },
	} });
	return { useScreens, useScreensOptional: useScreens };
});
vi.mock("../../features/playbackSolo/PlaybackSoloContext", () => ({
	usePlaybackSolo: () => ({ authorityGeneration: 1, activate: () => {}, subscribe: () => () => {},
		getRegions: () => mocks.regions, isSaving: () => false, error: null }),
}));
const editor: PlaybackSoloEditor = {
	available: true, ready: true, selecting: false, selected: [], name: "", saving: false,
	error: null, setName: () => {}, begin: () => mocks.begin(), beginEdit: () => {},
	save: async () => {}, cancel: () => {},
};
function Grid() {
	const registry = usePlaybackSoloEditorRegistry();
	useEffect(() => registry?.register("main", editor), [registry]);
	return <section aria-label="Main Playback grid" />;
}
function Command() {
	const host = usePlaybackSectionSettings();
	return <button onClick={host?.open}>Open section settings</button>;
}
function Main() {
	const [playbacks, setPlaybacks] = useState(false);
	return <PlaybackSoloEditorProvider>
		<PlaybackSectionSettingsHost surfaceId="main" onShowGrid={() => setPlaybacks(true)}>
			<Command />
			{playbacks ? <Grid /> : <section aria-label="Main Programmer" />}
		</PlaybackSectionSettingsHost>
	</PlaybackSoloEditorProvider>;
}
afterEach(cleanup);
beforeEach(() => { mocks.select.mockReset().mockResolvedValue(undefined); mocks.begin.mockClear(); });
it("keeps settings open while Programmer switches to the local grid, then creates a real Solo draft", async () => {
	render(<Main />);
	fireEvent.click(screen.getByRole("button", { name: "Open section settings" }));
	expect(screen.getByRole("region", { name: "Main Programmer" })).toBeInTheDocument();
	fireEvent.click(screen.getByRole("tab", { name: "Manage Solo Regions" }));
	await waitFor(() => expect(screen.getByRole("region", { name: "Main Playback grid" })).toBeInTheDocument());
	expect(mocks.select).toHaveBeenCalledExactlyOnceWith("main");
	expect(screen.getByRole("dialog", { name: "Playback section settings" })).toBeInTheDocument();
	const create = await screen.findByRole("button", { name: "Create New Solo Region" });
	expect(create).toBeEnabled();
	fireEvent.click(create);
	expect(mocks.begin).toHaveBeenCalledOnce();
	expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	expect(screen.getByRole("region", { name: "Main Playback grid" })).toBeInTheDocument();
});
it("shows selection failure without changing mode or closing settings", async () => {
	mocks.select.mockRejectedValue(new Error("Surface selection failed"));
	render(<Main />);
	fireEvent.click(screen.getByRole("button", { name: "Open section settings" }));
	fireEvent.click(screen.getByRole("tab", { name: "Manage Solo Regions" }));
	expect(await screen.findByRole("alert")).toHaveTextContent("Surface selection failed");
	expect(screen.getByRole("region", { name: "Main Programmer" })).toBeInTheDocument();
	expect(screen.queryByRole("region", { name: "Main Playback grid" })).not.toBeInTheDocument();
	expect(screen.getByRole("dialog")).toBeInTheDocument();
});
