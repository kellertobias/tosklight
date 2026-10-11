import { createRoot } from "react-dom/client";
import { TouchPlaybackCardView } from "@tosklight/ui/playback";
import { ScreenPlaybackFit } from "../../../apps/light-desktop/src/features/screens/ScreenPlaybackFit";
import "../../../apps/light-desktop/src/applicationStyles";

/** Actual renderer and cascade; fixture supplies authored tracks and idle card data. */
function Fixture() {
	return (
		<div className="playback-pane" style={{ width: "100vw", height: "100vh" }}>
			<ScreenPlaybackFit className="screen-bank-fit">
				<section className="playback-surface-controls">
					<div className="playback-bank-fixed">
						<div
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
						</div>
					</div>
				</section>
			</ScreenPlaybackFit>
		</div>
	);
}
createRoot(document.getElementById("root")!).render(<Fixture />);
