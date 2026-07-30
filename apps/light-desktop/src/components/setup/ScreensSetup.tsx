import { Button } from "@tosklight/ui";
import { useCallback, useEffect, useState } from "react";
import type {
	PlaybackSurfaceLayout,
	ScreenConfiguration,
} from "../../api/types";
import { useFiles } from "../../features/files/FilesContext";
import { useScreens } from "../../features/screens/ScreensContext";
import { useCueLists } from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import { listTextEditorFiles } from "../../windows/TextEditorWindow";
import { PlaybackLayoutModal } from "./PlaybackLayoutModal";
import {
	createScreenConfiguration,
	playbackLayoutLegacyFields,
} from "./screenConfiguration";
import { DefaultScreenPicker } from "./screens/DefaultScreenPicker";
import {
	DefaultScreenSettings,
	ScreenSettingsCard,
} from "./screens/ScreenSettingsCards";
import {
	type ScreenUndoHandle,
	useDefaultScreenDraft,
} from "./screens/useDefaultScreenDraft";

export { DefaultScreenPicker } from "./screens/DefaultScreenPicker";
export { ScreenSettingsCard } from "./screens/ScreenSettingsCards";

function ScreensSetupHeader({
	desktopAvailable,
	onCreate,
}: {
	desktopAvailable: boolean;
	onCreate: () => void;
}) {
	return (
		<header>
			<div>
				<h2>Screens & playback</h2>
				<p>
					Configure the default desk surface, installation-wide remote control,
					then optional operator screens.
				</p>
			</div>
			{desktopAvailable && (
				<Button variant="primary" onClick={onCreate}>
					+ Add screen
				</Button>
			)}
		</header>
	);
}

export function ScreensSetup({
	undoRef,
	onUndoAvailabilityChange,
}: {
	undoRef?: ScreenUndoHandle;
	onUndoAvailabilityChange?: (available: boolean) => void;
} = {}) {
	const server = useScreens();
	const files = useFiles();
	useShowObjectView("cue_list", true);
	const cueListObjects = useCueLists();
	const desktop = useDesktopBridge();
	const { state, dispatch } = useApp();
	const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>(
		[],
	);
	const [textFiles, setTextFiles] = useState<
		Array<{ root: string; rootLabel: string; path: string; name: string }>
	>([]);
	const [defaultScreenPickerOpen, setDefaultScreenPickerOpen] = useState(false);
	const [defaultPlaybackModalOpen, setDefaultPlaybackModalOpen] =
		useState(false);
	useEffect(() => {
		if (desktop.available) void desktop.listDisplays().then(setDisplays);
	}, [desktop]);
	useEffect(() => {
		if (!desktop.available) return;
		let cancelled = false;
		void files
			.fileRoots()
			.then(async (roots) => {
				const listings = await Promise.all(
					roots.map(async (root) => ({
						root,
						files: (await listTextEditorFiles(files.fileEntries, root.id))
							.files,
					})),
				);
				if (cancelled) return;
				setTextFiles(
					listings.flatMap(({ root, files: entries }) =>
						entries.map((entry) => ({
							root: root.id,
							rootLabel: root.label,
							path: entry.path,
							name: entry.name,
						})),
					),
				);
			})
			.catch(() => {
				if (!cancelled) setTextFiles([]);
			});
		return () => {
			cancelled = true;
		};
	}, [desktop.available, files]);
	const updateKeyboardShortcuts = useCallback(
		(value: boolean) =>
			dispatch({ type: "SET_REGULAR_NUMBER_SHORTCUTS", value }),
		[dispatch],
	);
	const updatePlaybackLayout = useCallback(
		(layout: PlaybackSurfaceLayout) =>
			dispatch({
				type: "SET_PLAYBACK_LAYOUT",
				columns: layout.playbacks_per_row,
				rows: layout.rows.length,
			}),
		[dispatch],
	);
	const defaultScreen = useDefaultScreenDraft({
		desk: server.session?.desk,
		regularNumberShortcuts: state.regularNumberShortcuts,
		onKeyboardShortcuts: updateKeyboardShortcuts,
		onPlaybackLayout: updatePlaybackLayout,
		onPersistDesk: server.updateControlDesk,
		undoRef,
		onUndoAvailabilityChange,
	});
	const create = () =>
		void server.saveScreen(
			createScreenConfiguration(server.screens?.screens ?? [], {
				desks: state.desks,
				activeDeskId: state.activeDeskId,
			}),
		);
	const remove = async (screen: ScreenConfiguration) => {
		await desktop.closeConsoleScreen(screen.id);
		await server.deleteScreen(screen.id);
	};
	return (
		<div className="screens-playback-setup">
			<ScreensSetupHeader
				desktopAvailable={desktop.available}
				onCreate={create}
			/>
			<div className="screens-setup-list">
				<DefaultScreenSettings
					deskName={defaultScreen.draft?.name ?? ""}
					deskAlias={defaultScreen.draft?.osc_alias ?? ""}
					playbackLayout={defaultScreen.playbackLayout}
					fallbackColumns={state.playbackColumns}
					fallbackRows={state.playbackRows}
					playbackSlots={state.playbackColumns * state.playbackRows}
					keyboardShortcuts={state.regularNumberShortcuts}
					onName={(name) => defaultScreen.updateText("name", name)}
					onAlias={(alias) => defaultScreen.updateText("osc_alias", alias)}
					onTextFocus={defaultScreen.beginTextEdit}
					onTextBlur={defaultScreen.endTextEdit}
					onKeyboardShortcuts={defaultScreen.updateKeyboardShortcuts}
					onConfigurePlaybacks={() => setDefaultPlaybackModalOpen(true)}
					onChooseDefault={() => setDefaultScreenPickerOpen(true)}
				/>
				{!desktop.available && (
					<p>
						Additional console screens are available in the ToskLight desktop
						app.
					</p>
				)}
				{desktop.available &&
					(server.screens?.screens ?? []).map((screen) => (
						<ScreenSettingsCard
							key={screen.id}
							screen={screen}
							desks={state.desks}
							displays={displays}
							cueLists={cueListObjects.map((cueList) => ({
								id: cueList.body.id,
								name: cueList.body.name,
							}))}
							textFiles={textFiles}
							save={server.saveScreen}
							remove={remove}
						/>
					))}
			</div>
			{defaultScreenPickerOpen && (
				<DefaultScreenPicker
					clients={server.bootstrap?.clients ?? []}
					currentClientId={server.session?.client_id}
					currentDeskId={server.session?.desk.id}
					onSelect={server.selectControlDesk}
					onRemove={server.removeClient}
					onClose={() => setDefaultScreenPickerOpen(false)}
				/>
			)}
			{defaultPlaybackModalOpen && defaultScreen.playbackLayout && (
				<PlaybackLayoutModal
					initialLayout={defaultScreen.playbackLayout}
					pageMode="follow_main"
					pageModeLocked
					onClose={() => setDefaultPlaybackModalOpen(false)}
					onSave={(layout) => {
						const legacy = playbackLayoutLegacyFields(layout);
						defaultScreen.updateDesk({
							columns: legacy.columns,
							rows: legacy.rows,
							buttons: legacy.buttons,
							playback_layout: layout,
						});
						setDefaultPlaybackModalOpen(false);
					}}
				/>
			)}
		</div>
	);
}
