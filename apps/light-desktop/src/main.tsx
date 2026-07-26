import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ModalProvider } from "@tosklight/ui/modals";
import { App } from "./App";
import { ScreenApp } from "./ScreenApp";
import { StageViewApp } from "./StageViewApp";
import "./applicationStyles";
import { enableSetOnContextMenu } from "./disableContextMenu";
import { ProductDemoApp } from "./ProductDemoApp";
import { createDesktopBridge, DesktopProvider } from "./platform/desktop";
import { SessionHandoffProvider } from "./features/session/SessionHandoffContext";
import { createSessionHandoff } from "./features/session/sessionHandoff";

enableSetOnContextMenu();
const desktop = createDesktopBridge();
const sessionHandoff = createSessionHandoff();
const screenId = new URLSearchParams(window.location.search).get("screen");
const stageView = new URLSearchParams(window.location.search).get("stage-view") === "1";
const productDemo = new URLSearchParams(window.location.search).get("demo") === "product";
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SessionHandoffProvider handoff={sessionHandoff}>
      <DesktopProvider bridge={desktop}>
        <ModalProvider>
          {productDemo ? <ProductDemoApp /> : stageView ? <StageViewApp /> : screenId ? <ScreenApp id={screenId}/> : <App />}
        </ModalProvider>
      </DesktopProvider>
    </SessionHandoffProvider>
  </StrictMode>,
);
