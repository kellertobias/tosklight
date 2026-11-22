// The application shell: navigation, the connection indicator, and the current page.
//
// Pages compose features. Nothing here converts a protocol or holds a state machine.

import { WindowFrame } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import { AudioPage } from "../features/audio/AudioPage";
import { PlaylistsPage } from "../features/automation/PlaylistsPage";
import { SchedulePage } from "../features/automation/SchedulePage";
import { DmxPage } from "../features/dmx/DmxPage";
import { MediaPanePage } from "../features/layers/MediaPanePage";
import { LibraryPage } from "../features/media-library/LibraryPage";
import { PixelMapPage } from "../features/pixelmap/PixelMapPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { DeskIdentityProvider } from "../operator/DeskIdentityContext";
import {
	PlaybackTakeoverProvider,
	PlaybackTakeoverToggle,
	usePlaybackTakeover,
} from "../operator/PlaybackTakeoverContext";
import {
	type MediaServerSection,
	MediaServerShell,
} from "../operator/MediaServerSurface";
import { useHealth } from "../shared/api/queries";
import { useTelemetry } from "../shared/api/telemetry";
import { usePreviewNavigation } from "../operator/usePreviewNavigation";
import { ErrorBoundary } from "./ErrorBoundary";
import { ROUTES, type RoutePath } from "./routes";
import { ToastProvider } from "./ToastContext";
import { useRouter } from "./useRouter";

// The connection indicator is the only thing on the shell that must stay live on every page.
const HEALTH_POLL_MS = 5_000;
/** The pages that offer Enable preview: the Library and the editors of what a layer can show. */
const PREVIEW_PATHS: ReadonlySet<string> = new Set([
	"/library",
	"/visualizers",
	"/text",
	"/effects",
	"/models",
]);

const PAGES: Record<RoutePath, () => React.ReactElement> = {
	"/": MediaPanePage,
	"/library": () => <LibraryPage />,
	"/playlists": PlaylistsPage,
	"/schedule": SchedulePage,
	"/visualizers": () => <LibraryPage mode="visualizers" />,
	"/text": () => <LibraryPage mode="text" />,
	"/effects": () => <LibraryPage mode="effects" />,
	"/models": () => <LibraryPage mode="models" />,
	"/audio": AudioPage,
	"/dmx": DmxPage,
	"/pixel-map": PixelMapPage,
	"/settings": SettingsPage,
};

const SECTION_BY_PATH: Record<RoutePath, MediaServerSection> = {
	"/": "media",
	"/library": "library",
	"/playlists": "playlists",
	"/schedule": "schedule",
	"/visualizers": "library",
	"/text": "library",
	"/effects": "library",
	"/models": "library",
	"/audio": "audio",
	"/dmx": "dmx",
	"/pixel-map": "pixel-map",
	"/settings": "settings",
};

export function App() {
	return (
		<ToastProvider>
			<PlaybackTakeoverProvider>
				<AppSurface />
			</PlaybackTakeoverProvider>
		</ToastProvider>
	);
}

function AppSurface() {
	const { path, navigate, headingRef } = useRouter();
	const health = useHealth(HEALTH_POLL_MS);
	const telemetry = useTelemetry();
	const showName = telemetry.frame?.deskIdentity?.showName;
	const Page = PAGES[path];
	const route =
		ROUTES.find((candidate) => candidate.path === path) ?? ROUTES[0];
	const pageOwnsWindow =
		path === "/" ||
		path === "/library" ||
		path === "/playlists" ||
		path === "/schedule" ||
		path === "/visualizers" ||
		path === "/text" ||
		path === "/effects" ||
		path === "/models" ||
		path === "/audio" ||
		path === "/dmx" ||
		path === "/pixel-map" ||
		path === "/settings";
	const libraryMode =
		path === "/models"
			? "models"
			: path === "/effects"
			? "effects"
			: path === "/visualizers"
			? "visualizers"
			: path === "/text"
				? "text"
				: "media";
	const { preview, playlistPreview } = usePlaybackTakeover();
	const leaving = usePreviewNavigation(playlistPreview.stop, navigate);
	// Preview belongs to the Library: leaving it puts the output back as it was.
	// Moving between previewing pages keeps preview on; any other page turns it off.
	// biome-ignore lint/correctness/useExhaustiveDependencies: only a change of page turns it off.
	useEffect(() => {
		if (!PREVIEW_PATHS.has(path)) void preview.setEnabled(false);
	}, [path]);
	const [now, setNow] = useState(() => new Date());
	useEffect(() => {
		const timer = window.setInterval(() => setNow(new Date()), 30_000);
		return () => window.clearInterval(timer);
	}, []);

	return (
		<MediaServerShell
			active={SECTION_BY_PATH[path]}
			connected={health.data !== undefined && health.failure === undefined}
			instance={health.data?.instance}
			showName={showName}
			now={now}
			playbackOwnership={<PlaybackTakeoverToggle preview={PREVIEW_PATHS.has(path)} />}
			onNavigate={(section) => {
				const route = ROUTES.find(
					(candidate) => SECTION_BY_PATH[candidate.path] === section,
				);
				if (route) void leaving.navigate(route.path);
			}}
		>
			<DeskIdentityProvider showName={showName}>
				{leaving.stopping && <p role="status">Stopping playlist preview…</p>}
				<div ref={headingRef} tabIndex={-1} className="media-route-surface" inert={leaving.stopping} aria-busy={leaving.stopping}>
					<ErrorBoundary key={path}>
						{pageOwnsWindow ? (
							path === "/library" ||
							path === "/visualizers" ||
							path === "/text" || path === "/effects" || path === "/models" ? (
								<LibraryPage
									mode={libraryMode}
									onModeChange={(mode) =>
										navigate(
											mode === "media"
												? "/library"
											: mode === "visualizers"
												? "/visualizers"
												: mode === "text"
													? "/text"
													: mode === "models"
														? "/models"
														: "/effects",
										)
									}
								/>
							) : (
								<Page />
							)
						) : (
							<WindowFrame
								title={route.label}
								info={{
									primary: "Media Server",
									secondary: "Operator controls",
								}}
								className="media-route-window"
							>
								<Page />
							</WindowFrame>
						)}
					</ErrorBoundary>
				</div>
			</DeskIdentityProvider>
		</MediaServerShell>
	);
}
