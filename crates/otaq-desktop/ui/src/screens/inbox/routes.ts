// The page's own stores, where opening an inbox item lands (kept apart so the tests pass their own).
import { overlays } from "../../app/overlays";
import { selection, terminalFocusRequests } from "../../app/selection";
import { tabRequests } from "../terminals/logic";
import type { OpenRoutes } from "./logic";

export const ROUTES: OpenRoutes = { selection, focus: terminalFocusRequests, overlays, tabs: tabRequests };
