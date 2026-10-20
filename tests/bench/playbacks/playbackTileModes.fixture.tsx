import "../../../apps/light-desktop/src/styles.css";

import { useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { SoloFenceOverlay } from "../../../apps/light-desktop/src/components/control/playbackFaderBank/SoloFenceOverlay.tsx";
import { fittedPlaybackGeometry } from "../../../apps/light-desktop/src/components/control/playbackFaderBank/tileGeometry.ts";
import { usePaneTileGeometry } from "../../../apps/light-desktop/src/components/control/playbackFaderBank/usePaneTileGeometry.ts";
import { ScreenPlaybackFit } from "../../../apps/light-desktop/src/features/screens/ScreenPlaybackFit";
import { PlaybackTileView } from "../../../apps/ui-library/src/playback/PlaybackTileView.tsx";

type TileMode = "automatic" | "fixed" | undefined;
const browserFixture = window as unknown as {
	setTileMode: (mode: TileMode) => void;
	actions: number;
};
const positions = [...Array.from({ length: 28 }, (_, i) => 401 + i), 800, 900];
const regions = [
	{
		id: "solo",
		name: "Touring pair",
		color: "#06b6d4",
		members: [
			{ page: 1, position: 401 },
			{ page: 1, position: 402 },
		],
	},
];
function App() {
	const viewport = useRef<HTMLDivElement>(null);
	const [bank, setBank] = useState<HTMLDivElement | null>(null);
	const [mode, setMode] = useState<TileMode>(undefined);
	browserFixture.setTileMode = setMode;
	browserFixture.actions = 0;
	const layout =
		mode === "automatic"
			? { mode }
			: mode === "fixed"
				? { mode, rows: 3, columns: 6 }
				: undefined;
	const geometry = usePaneTileGeometry(
		layout,
		viewport.current,
		bank,
		positions.length,
		{ defaultWidth: 144, minimumWidth: 100 },
	);
	const columns = geometry?.columns ?? 6;
	return (
		<div style={{ width: 900, height: 350 }}>
			<ScreenPlaybackFit>
				<section className="playback-surface-controls">
					<div
						ref={viewport}
						className={
							layout ? "playback-bank-viewport" : "playback-bank-fixed"
						}
						style={{
							overflow: layout ? "auto" : "hidden",
							scrollbarGutter: layout ? "stable" : undefined,
						}}
					>
						<div
							ref={setBank}
							className="playback-fader-bank touch-layout"
							data-playback-presentation="tiles"
							data-playback-surface="pane:fixture"
							style={
								geometry?.style ??
								fittedPlaybackGeometry(
									{ width: 900, height: 350, columnGap: 3, rowGap: 3 },
									6,
									5,
								)
							}
						>
							{positions.map((slot, i) => (
								<PlaybackTileView
									key={slot}
									model={{
										page: 1,
										slot,
										row: Math.floor(i / columns),
										rowUnits: 1,
										name: `Touring ${slot}`,
										assigned: true,
										kind: "cue-list",
										presentation: "buttons",
										style: {
											gridColumn: (i % columns) + 1,
											gridRow: Math.floor(i / columns) + 1,
										},
										hasFader: false,
										faderValue: 0,
										faderLabel: "Master",
										faderDisplay: "OFF",
										actions: [],
									}}
									action={{
										id: "go",
										label: "Go",
										onClick: () => browserFixture.actions++,
									}}
								/>
							))}
							<SoloFenceOverlay
								bank={bank}
								page={1}
								regions={regions}
								layoutKey={columns}
							/>
						</div>
					</div>
				</section>
			</ScreenPlaybackFit>
		</div>
	);
}
const app = document.getElementById("app");
if (!app) throw new Error("Fixture root missing");
createRoot(app).render(<App />);
