import { ModalProvider } from "@tosklight/ui/modals";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { CadApp } from "./cad/CadApp";
import "@tosklight/ui/styles.css";
import "@tosklight/patch/styles.css";
import "./styles.css";

const surface = new URLSearchParams(window.location.search).get("surface");

createRoot(document.getElementById("root")!).render(
	<StrictMode>
		<ModalProvider>
			{surface === "cad" ? <CadApp /> : <App />}
		</ModalProvider>
	</StrictMode>,
);
