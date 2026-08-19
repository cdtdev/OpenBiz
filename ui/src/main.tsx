import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
// The design system, loaded once at the entry point. Every rule the interface uses hangs off
// the tokens this pulls in; see `design/tokens.css`.
import "./app.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("root element missing from index.html");
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
