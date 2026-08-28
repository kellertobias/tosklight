import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useState } from "react";
import { FixturePatchSetupContent } from "../components/setup/FixturePatchSetup";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { PsnSetup } from "../components/setup/PsnSetup";
import { PatchFeatureBoundary } from "../features/patch/PatchFeatureBoundary";
import { useDesktopBridge } from "../platform/desktop";
import type { WindowProps } from "./windowTypes";

export function PatchWindow({ active = true, patchView = "fixtures" }: WindowProps) {
	const [tab, setTab] = useState<"fixtures" | "media" | "tracking">(patchView);
	return (
		<PatchFeatureBoundary>
			{tab === "media" && (
				<PatchMediaWindow
					active={active}
					onFixtures={() => setTab("fixtures")}
					onTracking={() => setTab("tracking")}
				/>
			)}
			{tab === "tracking" && (
				<PatchTrackingWindow
					active={active}
					onFixtures={() => setTab("fixtures")}
					onMedia={() => setTab("media")}
				/>
			)}
			{tab === "fixtures" && (
				<PatchWindowContent
					active={active}
					onMedia={() => setTab("media")}
					onTracking={() => setTab("tracking")}
				/>
			)}
		</PatchFeatureBoundary>
	);
}

function PatchWindowContent({
	active,
	onMedia,
	onTracking,
}: {
	active: boolean;
	onMedia: () => void;
	onTracking: () => void;
}) {
	const desktop = useDesktopBridge();
	const [rendererError, setRendererError] = useState<string | null>(null);
	const openStageRenderer = async () => {
		setRendererError(null);
		try {
			await desktop.openVisualizer();
		} catch (error) {
			setRendererError(
				error instanceof Error
					? error.message
					: "The Stage renderer could not be opened.",
			);
		}
	};
	return (
		<div className="patch-window">
			<FixturePatchSetupContent
				active={active}
				onMedia={onMedia}
				onTracking={onTracking}
				onOpenStageWindow={desktop.available ? openStageRenderer : undefined}
			/>
			{rendererError && <p role="alert">{rendererError}</p>}
		</div>
	);
}

function PatchMediaWindow({
	active,
	onFixtures,
	onTracking,
}: {
	active: boolean;
	onFixtures: () => void;
	onTracking: () => void;
}) {
	return (
		<>
			<WindowHeader
				title="Show Patch"
				info={{ primary: "Media Servers" }}
				groups={[
					{
						id: "patch-kind",
						kind: "tabs",
						activeId: "media",
						onActiveChange: (id) => {
							if (id === "fixtures") onFixtures();
							if (id === "tracking") onTracking();
						},
						actions: [
							{ id: "fixtures", label: "Fixtures" },
							{ id: "media", label: "Media Servers" },
							{ id: "tracking", label: "Tracking" },
						],
					},
				]}
			/>
			<WindowScrollArea>
				<main>
					<MediaServerSetup active={active} />
				</main>
			</WindowScrollArea>
		</>
	);
}

/**
 * Tracking as a screen of the Show Patch.
 *
 * It sits beside Fixtures and Media Servers because that is what it is: part of setting the show
 * up, done once with the rig, not something reached for while a show is running.
 */
function PatchTrackingWindow({
	active,
	onFixtures,
	onMedia,
}: {
	active: boolean;
	onFixtures: () => void;
	onMedia: () => void;
}) {
	return (
		<>
			<WindowHeader
				title="Show Patch"
				info={{ primary: "Tracking" }}
				groups={[
					{
						id: "patch-kind",
						kind: "tabs",
						activeId: "tracking",
						onActiveChange: (id) => {
							if (id === "fixtures") onFixtures();
							if (id === "media") onMedia();
						},
						actions: [
							{ id: "fixtures", label: "Fixtures" },
							{ id: "media", label: "Media Servers" },
							{ id: "tracking", label: "Tracking" },
						],
					},
				]}
			/>
			<WindowScrollArea>
				<main>
					<PsnSetup active={active} />
				</main>
			</WindowScrollArea>
		</>
	);
}
