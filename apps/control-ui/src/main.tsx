import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ScreenApp } from "./ScreenApp";
import { StageViewApp } from "./StageViewApp";
import "./styles.css";
import "./help.css";
import "./window-kit.css";
import "./hardware.css";
import "./chrome.css";
import "./hardware-dense.css";
import "./workflow-themes.css";
import "./playback-colors.css";
import "./fixture-address.css";
import "./cuelist-settings-layout.css";
import { UiKitCatalog } from "./components/window-kit/UiKitCatalog";
import { enableSetOnContextMenu } from "./disableContextMenu";
import { ProductDemoApp } from "./ProductDemoApp";
import "./product-demo.css";
import { createDesktopBridge, DesktopProvider } from "./platform/desktop";
import { SessionHandoffProvider } from "./features/session/SessionHandoffContext";
import { createSessionHandoff } from "./features/session/sessionHandoff";

enableSetOnContextMenu();
const desktop = createDesktopBridge();
const sessionHandoff = createSessionHandoff();
const screenId = new URLSearchParams(window.location.search).get("screen");
const stageView = new URLSearchParams(window.location.search).get("stage-view") === "1";
const uiKit = import.meta.env.DEV && new URLSearchParams(window.location.search).get("ui-kit") === "1";
const productDemo = new URLSearchParams(window.location.search).get("demo") === "product";
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SessionHandoffProvider handoff={sessionHandoff}>
      <DesktopProvider bridge={desktop}>
        {productDemo ? <ProductDemoApp /> : uiKit ? <UiKitCatalog /> : stageView ? <StageViewApp /> : screenId ? <ScreenApp id={screenId}/> : <App />}
      </DesktopProvider>
    </SessionHandoffProvider>
  </StrictMode>,
);
