import { ModalProvider } from "@tosklight/ui/modals";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ScreenApp } from "./ScreenApp";
import { StageViewApp } from "./StageViewApp";
import "./applicationStyles";
import { SessionHandoffProvider } from "./features/session/SessionHandoffContext";
import { createSessionHandoff } from "./features/session/sessionHandoff";
import { PackagedImprovedBeamSpikeApp } from "./PackagedImprovedBeamSpikeApp";
import { PackagedStageBenchmarkApp } from "./PackagedStageBenchmarkApp";
import { ProductDemoApp } from "./ProductDemoApp";
import { createDesktopBridge, DesktopProvider } from "./platform/desktop";

const desktop = createDesktopBridge();
const packagedStageBenchmark = desktop.available
	? await desktop.packagedStageBenchmarkConfig().catch(() => null)
	: null;
const sessionHandoff = createSessionHandoff();
const screenId = new URLSearchParams(window.location.search).get("screen");
const stageView =
	new URLSearchParams(window.location.search).get("stage-view") === "1";
const productDemo =
	new URLSearchParams(window.location.search).get("demo") === "product";
createRoot(document.getElementById("root")!).render(
	<StrictMode>
		<SessionHandoffProvider handoff={sessionHandoff}>
			<DesktopProvider bridge={desktop}>
				<ModalProvider>
					{stageView ? (
						<StageViewApp />
					) : packagedStageBenchmark?.profile === "improved-beam-spike" ? (
						<PackagedImprovedBeamSpikeApp />
					) : packagedStageBenchmark ? (
						<PackagedStageBenchmarkApp
							durationSeconds={packagedStageBenchmark.durationSeconds}
							controlDurationSeconds={
								packagedStageBenchmark.controlDurationSeconds
							}
							profile={packagedStageBenchmark.profile}
							additionalStageWindow={
								packagedStageBenchmark.additionalStageWindow
							}
							fixtureSheet={packagedStageBenchmark.fixtureSheet}
						/>
					) : productDemo ? (
						<ProductDemoApp />
					) : screenId ? (
						<ScreenApp id={screenId} />
					) : (
						<App />
					)}
				</ModalProvider>
			</DesktopProvider>
		</SessionHandoffProvider>
	</StrictMode>,
);
