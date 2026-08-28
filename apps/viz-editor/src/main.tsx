import { ModalProvider } from "@tosklight/ui/modals";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "@tosklight/ui/styles.css";
import "@tosklight/patch/styles.css";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
	<StrictMode>
		<ModalProvider>
			<App />
		</ModalProvider>
	</StrictMode>,
);
