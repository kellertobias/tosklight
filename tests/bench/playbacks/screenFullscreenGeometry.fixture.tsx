import { createRoot } from "react-dom/client";
import { TouchPlaybackCardView } from "@tosklight/ui/playback";
import { NativeDragStrip } from "../../../apps/light-desktop/src/components/shell/NativeDragStrip";
import { DesktopProvider } from "../../../apps/light-desktop/src/platform/desktop/DesktopContext";
import { browserDesktopBridge } from "../../../apps/light-desktop/src/platform/desktop/browserDesktopBridge";
import { ScreenPlaybackDraftLayout } from "../../../apps/light-desktop/src/features/screens/ScreenPlaybackDraftLayout";
import { ScreenPlaybackFit } from "../../../apps/light-desktop/src/features/screens/ScreenPlaybackFit";
import "../../../apps/light-desktop/src/applicationStyles";

const parameters = new URLSearchParams(location.hash.slice(1));
const fullscreen = parameters.get("fullscreen") !== "false";
const rail = parameters.get("rail") !== "false";
const draft = parameters.get("draft") === "true";
const bridge = {
	...browserDesktopBridge,
	available: true,
	currentWindowFullscreen: async () => fullscreen,
	setCurrentWindowFullscreen: async () => {},
};

/** Real production wrapper, chrome, renderer and cascade; no runtime actions. */
function Fixture() {
	return (
		<DesktopProvider bridge={bridge}>
			<div className="screen-shell utility-content with-playbacks">
				<NativeDragStrip compactFullscreen />
				<ScreenPlaybackFit className="screen-bank-fit">
					<ScreenPlaybackDraftLayout
						editor={draft ? <div style={{ height: 44 }}>Solo draft</div> : null}
					>
						<section
							className={`screen-playbacks ${rail ? "" : "playbacks-only"}`}
						>
							<section className="playback-surface-controls">
								<div className="playback-bank-fixed">
									<div
										className="playback-fader-bank touch-layout"
										style={{
											gridTemplateColumns: "repeat(10, minmax(0, 1fr))",
											gridTemplateRows: "minmax(0, 2fr) minmax(0, 1fr)",
										}}
									>
										{Array.from({ length: 20 }, (_, index) => (
											<TouchPlaybackCardView
												key={index}
												model={{
													page: 2,
													slot: index + 1,
													row: index < 10 ? 0 : 1,
													rowUnits: index < 10 ? 2 : 1,
													assigned: true,
													kind: "cue-list",
													name: `Playback ${index + 1}`,
													hasFader: index < 10,
													presentation: index < 10 ? "fader" : "buttons",
													faderValue: 100,
													faderLabel: "Master",
													faderDisplay: "100%",
													actions: [{ id: "go", label: "GO" }],
												}}
											/>
										))}
									</div>
								</div>
							</section>
							{rail && (
								<div className="screen-page-controls">
									<button>PAGE UP</button>
									<button>Page 2</button>
									<button>PAGE DOWN</button>
								</div>
							)}
						</section>
					</ScreenPlaybackDraftLayout>
				</ScreenPlaybackFit>
			</div>
		</DesktopProvider>
	);
}
createRoot(document.getElementById("root")!).render(<Fixture />);
