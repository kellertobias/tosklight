import { useEffect, useRef, useState } from "react";
import type {
	ControlDesk,
	PlaybackSurfaceLayout,
	ScreenConfiguration,
} from "../../api/types";
import { useScreens } from "../../features/screens/ScreensContext";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import { PlaybackLayoutModal } from "./PlaybackLayoutModal";
import {
	createScreenConfiguration,
	defaultDeskPlaybackLayout,
	playbackLayoutLegacyFields,
} from "./screenConfiguration";
import { DefaultScreenPicker } from "./screens/DefaultScreenPicker";
import {
	DefaultScreenSettings,
	ScreenSettingsCard,
} from "./screens/ScreenSettingsCards";

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

export function ScreensSetup() {
	const server = useScreens();
	const desktop = useDesktopBridge();
	const { state, dispatch } = useApp();
	const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>(
		[],
	);
	const [deskPlaybackLayout, setDeskPlaybackLayout] =
		useState<PlaybackSurfaceLayout | null>(null);
	const [deskName, setDeskName] = useState("");
	const [deskAlias, setDeskAlias] = useState("");
	const [defaultScreenPickerOpen, setDefaultScreenPickerOpen] = useState(false);
	const [defaultPlaybackModalOpen, setDefaultPlaybackModalOpen] =
		useState(false);
	const deskDraft = useRef<ControlDesk | null>(null);
	const deskSaveQueue = useRef(Promise.resolve());
	const pendingDeskSaves = useRef(0);
	useEffect(() => {
		if (desktop.available) void desktop.listDisplays().then(setDisplays);
	}, [desktop]);
	useEffect(() => {
		const desk = server.session?.desk;
		if (!desk || pendingDeskSaves.current > 0) return;
		deskDraft.current = desk;
		setDeskName(desk.name);
		setDeskAlias(desk.osc_alias);
		setDeskPlaybackLayout(defaultDeskPlaybackLayout(desk));
		dispatch({
			type: "SET_PLAYBACK_LAYOUT",
			columns: desk.columns,
			rows: desk.rows,
		});
	}, [server.session?.desk, dispatch]);
	const applyDesk = (next: ControlDesk) => {
		const current = deskDraft.current ?? server.session?.desk;
		if (!current || JSON.stringify(current) === JSON.stringify(next)) {
			return false;
		}
		deskDraft.current = next;
		setDeskName(next.name);
		setDeskAlias(next.osc_alias);
		const layout = defaultDeskPlaybackLayout(next);
		setDeskPlaybackLayout(layout);
		dispatch({
			type: "SET_PLAYBACK_LAYOUT",
			columns: layout.playbacks_per_row,
			rows: layout.rows.length,
		});
		pendingDeskSaves.current += 1;
		deskSaveQueue.current = deskSaveQueue.current
			.then(() => server.updateControlDesk(next))
			.finally(() => {
				pendingDeskSaves.current -= 1;
			});
		return true;
	};
	const updateDesk = (changes: Partial<ControlDesk>) => {
		const current = deskDraft.current ?? server.session?.desk;
		return current ? applyDesk({ ...current, ...changes }) : false;
	};
	const updateText = (field: "name" | "osc_alias", value: string) => {
		updateDesk({ [field]: value });
	};
	const updateKeyboardShortcuts = (value: boolean) => {
		if (value === state.regularNumberShortcuts) return;
		dispatch({ type: "SET_REGULAR_NUMBER_SHORTCUTS", value });
	};
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
					deskName={deskName}
					deskAlias={deskAlias}
					playbackLayout={deskPlaybackLayout}
					fallbackColumns={state.playbackColumns}
					fallbackRows={state.playbackRows}
					playbackSlots={state.playbackColumns * state.playbackRows}
					keyboardShortcuts={state.regularNumberShortcuts}
					onName={(name) => updateText("name", name)}
					onAlias={(alias) => updateText("osc_alias", alias)}
					onKeyboardShortcuts={updateKeyboardShortcuts}
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
							displays={displays}
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
			{defaultPlaybackModalOpen && deskPlaybackLayout && (
				<PlaybackLayoutModal
					initialLayout={deskPlaybackLayout}
					pageMode="follow_main"
					pageModeLocked
					onClose={() => setDefaultPlaybackModalOpen(false)}
					onSave={(layout) => {
						const legacy = playbackLayoutLegacyFields(layout);
						updateDesk({
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
