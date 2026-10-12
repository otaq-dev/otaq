// The screens index: one line per screen, `export { <area> } from "./<area>";`, nothing else. The page runs every
// screen exported here once at start (`registerScreens` in `main.tsx`). See the crate README, "How a screen plugs in";
// `_sample/` is the template (not listed: the registry tests add it).
export { board } from "./board";
export { components } from "./components";
export { diff } from "./diff";
export { notices } from "./notices";
export { onboarding } from "./onboarding";
export { panel } from "./panel";
export { settings } from "./settings";
export { shell } from "./shell";
export { terminals } from "./terminals";
export { titleBar } from "./title_bar";
