import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ScreenApp } from "./ScreenApp";
import "./styles.css";
import "./help.css";
import "./window-kit.css";
import "./hardware.css";
import "./chrome.css";
import "./hardware-dense.css";
import { UiKitCatalog } from "./components/window-kit/UiKitCatalog";

const screenId = new URLSearchParams(window.location.search).get("screen");
const uiKit = import.meta.env.DEV && new URLSearchParams(window.location.search).get("ui-kit") === "1";
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {uiKit ? <UiKitCatalog /> : screenId ? <ScreenApp id={screenId}/> : <App />}
  </StrictMode>,
);
