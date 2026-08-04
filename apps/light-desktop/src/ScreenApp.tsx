import {
	type CSSProperties,
	type MutableRefObject,
	useEffect,
	useRef,
	useState,
} from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import type { ScreenConfiguration } from "./api/types";
import { LoadingSurface } from "./components/common/LoadingSurface";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { LeftDock } from "./components/shell/LeftDock";
import { NativeDragStrip } from "./components/shell/NativeDragStrip";
import { WorkspaceView } from "./components/shell/WorkspaceView";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { FixedScreenPane } from "./features/screens/FixedScreenPane";
import { ProgrammerControlSurfaceRegion } from "./features/screens/ProgrammerControlSurfaceRegion";
import { ScreenPlaybackSection } from "./features/screens/ScreenPlaybackSection";
import { useScreens } from "./features/screens/ScreensContext";
import { useScreenWindowPersistence } from "./platform/desktop";
import { AppProvider, useApp } from "./state/AppContext";

function DesktopScreenSurface({
	screen,
	saveScreen,
	closing,
}: {
	screen: ScreenConfiguration;
	saveScreen: (screen: ScreenConfiguration) => Promise<void>;
	closing: MutableRefObject<boolean>;
}) {
	const { state, dispatch } = useApp();
	const programmerOwner =
		useScreens().screens?.programmer_control_surface?.owner_screen_id ===
		screen.id;
	const showScreenControls =
		!programmerOwner && (screen.show_playbacks || screen.show_page_controls);
	const hydrated = useRef(false);
	const sidePane =
		screen.content.type === "fixed_side_pane" ? screen.content : null;
	const hasDesktop = !sidePane || sidePane.base === "desktop";
	const embeddedControl =
		sidePane?.base === "control_surface" && programmerOwner;
	const [layoutReady, setLayoutReady] = useState(!hasDesktop);
	const screenRef = useRef(screen);
	screenRef.current = screen;
	useEffect(() => {
		if (!hasDesktop) {
			hydrated.current = false;
			setLayoutReady(true);
			return;
		}
		if (hydrated.current) return;
		setLayoutReady(false);
		dispatch({
			type: "HYDRATE_LAYOUT",
			desks: screen.layout.desks,
			activeDeskId: screen.layout.activeDeskId,
		});
		hydrated.current = true;
		setLayoutReady(true);
	}, [screen, dispatch, hasDesktop]);
	useEffect(() => {
		if (!hasDesktop) return;
		const currentScreen = screenRef.current;
		if (!currentScreen || !hydrated.current || closing.current) return;
		const timer = window.setTimeout(() => {
			const latest = screenRef.current;
			if (latest && !closing.current)
				void saveScreen({
					...latest,
					layout: { desks: state.desks, activeDeskId: state.activeDeskId },
				});
		}, 600);
		return () => window.clearTimeout(timer);
	}, [state.desks, state.activeDeskId, hasDesktop]);
	if (hasDesktop && (!layoutReady || !hydrated.current))
		return (
			<LoadingSurface
				className="connection-cover"
				showMark
				title="Loading screen…"
				detail="Hydrating the assigned desktop and Playback surface"
			/>
		);
	return (
		<div
			className={`screen-shell ${screen.show_dock ? "with-dock" : ""} ${showScreenControls ? "with-playbacks" : ""} ${programmerOwner && !embeddedControl ? "with-control" : ""}`}
		>
			<NativeDragStrip />
			{screen.show_dock && <LeftDock />}
			{sidePane ? (
				<div
					className={`screen-main-composition fixed-${sidePane.side}`}
					style={
						{
							"--fixed-side-pane-width": `${sidePane.width_px}px`,
						} as CSSProperties
					}
				>
					<FixedScreenPane pane={sidePane.pane} />
					{sidePane.base === "desktop" ? (
						<WorkspaceView />
					) : embeddedControl ? (
						<div className="screen-main-base">
							<ProgrammerControlSurfaceRegion screenId={screen.id} />
						</div>
					) : sidePane.base === "control_surface" && !programmerOwner ? (
						<div className="screen-main-base parameter-empty" role="status">
							<b>Control surface is assigned elsewhere</b>
							<small>Assign this screen in Screens & playback.</small>
						</div>
					) : null}
				</div>
			) : (
				<WorkspaceView />
			)}
			{showScreenControls && <ScreenPlaybackSection screen={screen} />}
			{programmerOwner && !embeddedControl && (
				<ProgrammerControlSurfaceRegion screenId={screen.id} />
			)}
		</div>
	);
}

function FixedScreenSurface({ screen }: { screen: ScreenConfiguration }) {
	const programmerOwner =
		useScreens().screens?.programmer_control_surface?.owner_screen_id ===
		screen.id;
	if (screen.content.type !== "fixed_pane") return null;
	const showScreenControls =
		!programmerOwner && (screen.show_playbacks || screen.show_page_controls);
	return (
		<div
			className={`screen-shell fixed-content ${showScreenControls ? "with-playbacks" : ""} ${programmerOwner ? "with-control" : ""}`}
		>
			<NativeDragStrip />
			<FixedScreenPane pane={screen.content.pane} />
			{showScreenControls && <ScreenPlaybackSection screen={screen} />}
			{programmerOwner && (
				<ProgrammerControlSurfaceRegion screenId={screen.id} />
			)}
		</div>
	);
}

function UtilityScreenSurface({ screen }: { screen: ScreenConfiguration }) {
	const programmerOwner =
		useScreens().screens?.programmer_control_surface?.owner_screen_id ===
		screen.id;
	const showScreenControls =
		!programmerOwner && (screen.show_playbacks || screen.show_page_controls);
	return (
		<div
			className={`screen-shell utility-content ${showScreenControls ? "with-playbacks" : ""} ${programmerOwner ? "with-control" : ""}`}
		>
			<NativeDragStrip />
			{screen.content.type === "control_surface" && !programmerOwner ? (
				<div className="parameter-empty" role="status">
					<b>Control surface is assigned elsewhere</b>
					<small>Assign this screen in Screens & playback.</small>
				</div>
			) : null}
			{showScreenControls && <ScreenPlaybackSection screen={screen} />}
			{programmerOwner && (
				<ProgrammerControlSurfaceRegion screenId={screen.id} />
			)}
		</div>
	);
}

function ScreenSurface({ id }: { id: string }) {
	const server = useScreens();
	const screen = server.screens?.screens.find((item) => item.id === id);
	const closing = useScreenWindowPersistence(screen, server.saveScreen);
	if (server.screens && !screen)
		return (
			<div className="connection-cover parameter-empty" role="alert">
				<b>Screen unavailable</b>
				<small>
					This screen was removed or this browser link is not authorized for
					the current desk.
				</small>
			</div>
		);
	if (!screen)
		return (
			<LoadingSurface
				className="connection-cover"
				showMark
				title="Loading screen…"
				detail="Waiting for the assigned screen configuration"
			/>
		);
	if (screen.content.type === "fixed_pane")
		return <FixedScreenSurface screen={screen} />;
	if (
		screen.content.type === "control_surface" ||
		screen.content.type === "none"
	)
		return <UtilityScreenSurface screen={screen} />;
	return (
		<DesktopScreenSurface
			screen={screen}
			saveScreen={server.saveScreen}
			closing={closing}
		/>
	);
}

export function ScreenApp({ id }: { id: string }) {
	return (
		<ServerRuntime sessionRole="secondary">
			<AppProvider>
				<PatchFeatureBoundary>
					<ScreenSurface id={id} />
					<ConnectionState />
					<DeskLoadingOverlay />
				</PatchFeatureBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerRuntime>
	);
}
