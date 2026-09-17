import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import {
  createHttpNativeHardwareBridge,
  unavailableNativeHardwareBridge,
} from "./transport/nativeBridge";
import { createOscBridge, tauriOscBridge } from "./transport/oscBridge";
import "./styles.css";

const oscBridge = createOscBridge();
// An injected browser test port stands in for the desk, so there is no extension host to read.
const nativeBridge =
  oscBridge === tauriOscBridge
    ? createHttpNativeHardwareBridge()
    : unavailableNativeHardwareBridge;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App bridge={oscBridge} nativeBridge={nativeBridge} />
  </React.StrictMode>,
);
