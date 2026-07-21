import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { createOscBridge } from "./transport/oscBridge";
import "./styles.css";

const oscBridge = createOscBridge();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App bridge={oscBridge} />
  </React.StrictMode>,
);
