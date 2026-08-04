import { Button, FormLayout, SelectField } from "@tosklight/ui";
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

function useScreenResourceOptions() {
	const files = useFiles();
	const desktop = useDesktopBridge();
	const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>(
		[],
	);
	const [textFiles, setTextFiles] = useState<
		Array<{ root: string; rootLabel: string; path: string; name: string }>
	>([]);
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
	return { desktop, displays, textFiles };
}

export function ProgrammerControlSurfaceSettings() {
	const server = useScreens();
	const configuration = server.screens?.programmer_control_surface;
	if (!configuration) return null;
	const owner = server.screens?.screens.find(
		(screen) => screen.id === configuration.owner_screen_id,
	);
	const ownerClosed = Boolean(owner && !owner.desired_open);
	return (
		<section className="screen-settings-card programmer-control-surface-settings">
			<header>
				<div>
					<h3>Programmer control surface</h3>
					<p>
						Choose the one on-screen owner and how many software encoders it
						shows. Attached hardware remains six encoders.
					</p>
				</div>
			</header>
			<FormLayout columns={2} minColumnWidth={180}>
				<SelectField
					label="Control surface"
					value={configuration.owner_screen_id ?? "main"}
					onChange={(owner) =>
						void server.updateProgrammerControlSurface(
							owner === "main"
								? { assign_to_main: true }
								: { owner_screen_id: owner },
						)
					}
					options={[
						{ value: "main", label: "Main screen" },
						...(server.screens?.screens ?? []).map((screen) => ({
							value: screen.id,
							label: screen.name,
						})),
					]}
				/>
				<SelectField
					label="Visible encoders"
					value={String(configuration.visible_encoders)}
					onChange={(value) =>
						void server.updateProgrammerControlSurface({
							visible_encoders: value === "4" ? 4 : 6,
						})
					}
					options={[
						{ value: "4", label: "Four" },
						{ value: "6", label: "Six" },
					]}
				/>
			</FormLayout>
			{ownerClosed && (
				<div className="programmer-control-owner-warning" role="alert">
					<span>
						{owner?.name} is closed, so its Programmer control surface is not
						currently visible.
					</span>
					<Button
						variant="warning"
						onClick={() =>
							void server.updateProgrammerControlSurface({
								assign_to_main: true,
							})
						}
					>
						Return controls to main screen
					</Button>
				</div>
			)}
		</section>
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
	useShowObjectView("cue_list", true);
	const cueListObjects = useCueLists();
	const { desktop, displays, textFiles } = useScreenResourceOptions();
	const { state, dispatch } = useApp();
	const [defaultScreenPickerOpen, setDefaultScreenPickerOpen] = useState(false);
	const [defaultPlaybackModalOpen, setDefaultPlaybackModalOpen] =
		useState(false);
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
				<ProgrammerControlSurfaceSettings />
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
