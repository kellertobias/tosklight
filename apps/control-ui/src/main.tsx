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
import "./workflow-themes.css";
import "./playback-colors.css";
import "./fixture-address.css";
import "./cuelist-settings-layout.css";
import { UiKitCatalog } from "./components/window-kit/UiKitCatalog";
import { enableSetOnContextMenu } from "./disableContextMenu";
import { ProductDemoApp } from "./ProductDemoApp";
import "./product-demo.css";

enableSetOnContextMenu();
const screenId = new URLSearchParams(window.location.search).get("screen");
const uiKit = import.meta.env.DEV && new URLSearchParams(window.location.search).get("ui-kit") === "1";
const productDemo = new URLSearchParams(window.location.search).get("demo") === "product";
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {productDemo ? <ProductDemoApp /> : uiKit ? <UiKitCatalog /> : screenId ? <ScreenApp id={screenId}/> : <App />}
  </StrictMode>,
);
