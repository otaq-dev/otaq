// The screens index: one line per screen, `export { <area> } from "./<area>";`, nothing else. The page runs every
// screen exported here once at start (`registerScreens` in `main.tsx`). See the crate README, "How a screen plugs in";
// `_sample/` is the template (not listed: the registry tests add it).
export { board } from "./board";
export { components } from "./components";
export { diff } from "./diff";
export { editor } from "./editor";
export { inbox } from "./inbox";
export { notices } from "./notices";
export { onboarding } from "./onboarding";
export { panel } from "./panel";
export { pr } from "./pr";
export { recovery } from "./recovery";
export { review } from "./review";
export { run } from "./run";
export { sections } from "./sections";
export { settings } from "./settings";
export { shell } from "./shell";
export { terminalTab } from "./terminal_tab";
export { terminals } from "./terminals";
export { titleBar } from "./title_bar";
