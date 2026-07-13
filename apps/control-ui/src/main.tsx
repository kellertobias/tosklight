import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ScreenApp } from "./ScreenApp";
import "./styles.css";
import "./hardware.css";
import "./chrome.css";
import "./hardware-dense.css";

const screenId = new URLSearchParams(window.location.search).get("screen");
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {screenId ? <ScreenApp id={screenId}/> : <App />}
  </StrictMode>,
);
