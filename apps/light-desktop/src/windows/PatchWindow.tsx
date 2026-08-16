import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useState } from "react";
import { FixturePatchSetupContent } from "../components/setup/FixturePatchSetup";
import { MediaServerSetup } from "../components/setup/MediaServerSetup";
import { PatchFeatureBoundary } from "../features/patch/PatchFeatureBoundary";
import { useDesktopBridge } from "../platform/desktop";
import type { WindowProps } from "./windowTypes";

export function PatchWindow({ active = true, patchView = "fixtures" }: WindowProps) {
	const [tab, setTab] = useState<"fixtures" | "media">(patchView);
	return (
		<PatchFeatureBoundary>
			{tab === "media" ? (
				<PatchMediaWindow
					active={active}
					onFixtures={() => setTab("fixtures")}
				/>
			) : (
				<PatchWindowContent active={active} onMedia={() => setTab("media")} />
			)}
		</PatchFeatureBoundary>
	);
}

function PatchWindowContent({
	active,
	onMedia,
}: {
	active: boolean;
	onMedia: () => void;
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
				onOpenStageWindow={desktop.available ? openStageRenderer : undefined}
			/>
			{rendererError && <p role="alert">{rendererError}</p>}
		</div>
	);
}

function PatchMediaWindow({
	active,
	onFixtures,
}: {
	active: boolean;
	onFixtures: () => void;
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
						onActiveChange: (id) => id === "fixtures" && onFixtures(),
						actions: [
							{ id: "fixtures", label: "Fixtures" },
							{
								id: "media",
								label: "Media Servers",
							},
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
