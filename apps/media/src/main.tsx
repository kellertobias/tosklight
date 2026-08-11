import { ModalProvider } from "@tosklight/ui/modals";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import "@tosklight/ui/styles.css";
import "./styles.css";
import "./operator/mediaServerSurface.css";

const root = document.getElementById("root");
if (!root) throw new Error("the application shell is missing its mount point");

createRoot(root).render(
	<StrictMode>
		<ModalProvider>
			<App />
		</ModalProvider>
	</StrictMode>,
);
