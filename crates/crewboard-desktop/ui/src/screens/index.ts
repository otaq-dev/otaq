// The screens index: one line per screen, `export { <area> } from "./<area>";`, nothing else. The page runs every
// screen exported here once at start (`registerScreens` in `main.tsx`). See the crate README, "How a screen plugs in";
// `_sample/` is the template (not listed: the registry tests add it).
export { components } from "./components";
export { titleBar } from "./title_bar";
