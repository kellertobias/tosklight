import { useState } from "react";
import { createRoot } from "react-dom/client";
import { TouchPlaybackCardView } from "@tosklight/ui/playback";
import { ScreenPlaybackFit } from "../../../apps/light-desktop/src/features/screens/ScreenPlaybackFit";
import "../../../apps/light-desktop/src/applicationStyles";
import { SoloFenceOverlay } from "../../../apps/light-desktop/src/components/control/playbackFaderBank/SoloFenceOverlay";

const regions = [{
	id: "11111111-1111-4111-8111-111111111111",
	name: "Visible pair",
	members: [{ page: 2, position: 1 }, { page: 2, position: 2 }],
}];

/** Actual renderer and cascade; fixture supplies authored tracks and idle card data. */
function Fixture() {
	const [bank, setBank] = useState<HTMLDivElement | null>(null);
	return (
		<div className="playback-pane" style={{ width: "100vw", height: "100vh" }}>
			<ScreenPlaybackFit className="screen-bank-fit">
				<section className="playback-surface-controls">
					<div className="playback-bank-fixed">
						<div
							ref={setBank}
							className="playback-fader-bank touch-layout"
							data-playback-surface="pane:test"
							style={{
								gridTemplateColumns: "repeat(15, minmax(0, 1fr))",
								gridTemplateRows: "minmax(0, 4fr) minmax(0, 1fr)",
							}}
						>
							{Array.from({ length: 30 }, (_, index) => {
								const fader = index < 15;
								return (
									<TouchPlaybackCardView
										key={index}
										model={{
											page: 2,
											slot: fader ? index + 1 : index + 86,
											row: fader ? 0 : 1,
											rowUnits: fader ? 4 : 1,
											name: `Cue ${index + 1}`,
											assigned: true,
											kind: "cue-list",
											className: fader ? "" : "playback-row-compact",
											presentation: fader ? "fader" : "buttons",
											hasFader: fader,
											faderValue: 100,
											faderLabel: "Master",
											faderDisplay: "100%",
											actions: (fader ? ["GO", "PAUSE", "OFF"] : ["GO"]).map((label) => ({
												id: `${index}-${label}`,
												label,
											})),
										}}
									/>
								);
							})}
							<SoloFenceOverlay bank={bank} page={2} regions={regions} layoutKey="screen-fixture" />
						</div>
					</div>
				</section>
			</ScreenPlaybackFit>
		</div>
	);
}
createRoot(document.getElementById("root")!).render(<Fixture />);
