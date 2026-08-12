import { ModalProvider } from "@tosklight/ui/modals";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ScreenApp } from "./ScreenApp";
import "./applicationStyles";
import { SessionHandoffProvider } from "./features/session/SessionHandoffContext";
import { createSessionHandoff } from "./features/session/sessionHandoff";
import { ProductDemoApp } from "./ProductDemoApp";
import { NativePackagedStageBenchmarkApp } from "./NativePackagedStageBenchmarkApp";
import { installDeskContextMenuPolicy } from "./platform/deskContextMenuPolicy";
import { createDesktopBridge, DesktopProvider } from "./platform/desktop";

const desktop = createDesktopBridge();
const packagedStageBenchmark = desktop.available
	? await desktop.packagedStageBenchmarkConfig().catch(() => null)
	: null;
const sessionHandoff = createSessionHandoff();
installDeskContextMenuPolicy(document);
const screenId = new URLSearchParams(window.location.search).get("screen");
const productDemo =
	new URLSearchParams(window.location.search).get("demo") === "product";
createRoot(document.getElementById("root")!).render(
	<StrictMode>
		<SessionHandoffProvider handoff={sessionHandoff}>
			<DesktopProvider bridge={desktop}>
				<ModalProvider>
					{packagedStageBenchmark ? (
						<NativePackagedStageBenchmarkApp config={packagedStageBenchmark} />
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
