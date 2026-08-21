import { ModalProvider } from "@tosklight/ui/modals";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { CadApp } from "./cad/CadApp";
import { surfaceFromLocation } from "./surface";
import "@tosklight/ui/styles.css";
import "@tosklight/patch/styles.css";
import "./styles.css";

const Surface = surfaceFromLocation(window.location.search) === "cad" ? CadApp : App;

createRoot(document.getElementById("root")!).render(
	<StrictMode>
		<ModalProvider>
			<Surface />
		</ModalProvider>
	</StrictMode>,
);
