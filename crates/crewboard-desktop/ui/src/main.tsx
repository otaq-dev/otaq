import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { registerScreens } from "./app/registry";
import * as screens from "./screens";
import "./styles.css";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App registries={registerScreens(screens)} />
    </StrictMode>,
  );
}
