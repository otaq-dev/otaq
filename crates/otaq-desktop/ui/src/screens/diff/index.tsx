// Area 07 · Diff (T-0225): the panel's Diff tab with its line comments. What it says is Rust's (`src/api/diff.rs`,
// `otaq-view`'s `diff_page.rs`); lines arrive highlighted by `highlight.rs`.
import type { Screen } from "../../app/registry";
import { DiffTab } from "./DiffTab";
import "./diff.css";

export const diff: Screen = (app) => {
  app.panelTabs.register("diff", { Component: DiffTab });
};
