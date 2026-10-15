// Code colours in code blocks (T-0254, d0078): highlight.js, through lowlight, reads the code, and its classes are
// mapped onto the Diff tab's tokens (otaq-view `highlight.rs`, `CodeToken`: keyword, string, number, type, comment,
// function), so code looks the same in both places and in every theme. Only the picker's languages are loaded.
//
// The lowlight instance handed to Tiptap is wrapped: each block comes back as flat spans with one `tok-<token>` class,
// which code.css colours with the diff's variables. A block with no language, or one the picker doesn't offer, stays
// plain: nothing guesses its language.

import bash from "highlight.js/lib/languages/bash";
import diff from "highlight.js/lib/languages/diff";
import ini from "highlight.js/lib/languages/ini";
import json from "highlight.js/lib/languages/json";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import yaml from "highlight.js/lib/languages/yaml";
import { createLowlight } from "lowlight";

import type { CodeToken } from "../../generated/CodeToken";

/** highlight.js's grammars for the picker's languages; `ts` and `tsx` are TypeScript's, `toml` is ini's alias. */
const lowlight = createLowlight({ bash, diff, ini, json, rust, typescript, yaml });

type Tree = ReturnType<typeof lowlight.highlight>;
type TreeNode = Tree["children"][number];

/** A run of code in one colour. */
export interface CodeSpan {
  text: string;
  token: CodeToken;
}

/** The token a highlight.js element's classes name, if any (`["hljs-title", "function_"]` → function). */
export function tokenOf(classes: readonly string[], language: string): CodeToken | null {
  const has = (name: string) => classes.includes(name);
  if (has("hljs-comment")) return "comment";
  if (has("hljs-string") || has("hljs-regexp")) return "string";
  if (has("hljs-number") || has("hljs-literal")) return "number";
  if (has("hljs-keyword")) return "keyword";
  if (has("hljs-title")) {
    if (has("function_")) return "function";
    if (has("class_")) return "type";
    return null;
  }
  if (has("hljs-type")) return "type";
  // Shell builtins (`echo`, `cd`) are commands; elsewhere builtins are types and globals (`Promise`, `Vec`).
  if (has("hljs-built_in")) return language === "bash" ? "function" : "type";
  return null;
}

/** `code` in `language` as coloured runs; the innermost element that names a token wins, as in the Diff tab. */
export function highlightCode(language: string, code: string): CodeSpan[] {
  const spans: CodeSpan[] = [];
  const walk = (nodes: readonly TreeNode[], token: CodeToken) => {
    for (const node of nodes) {
      if (node.type === "text") {
        const last = spans[spans.length - 1];
        if (last?.token === token) last.text += node.value;
        else spans.push({ text: node.value, token });
      } else if (node.type === "element") {
        walk(node.children, tokenOf(node.properties.className ?? [], language) ?? token);
      }
    }
  };
  walk(lowlight.registered(language) ? lowlight.highlight(language, code).children : [], "plain");
  if (spans.length === 0 && code !== "") spans.push({ text: code, token: "plain" });
  return spans;
}

function tree(spans: readonly CodeSpan[]): Tree {
  return {
    type: "root",
    children: spans.map(({ text, token }) =>
      token === "plain"
        ? { type: "text", value: text }
        : {
            type: "element",
            tagName: "span",
            properties: { className: [`tok-${token}`] },
            children: [{ type: "text", value: text }],
          },
    ),
  };
}

/** What Tiptap's code block reads its colours from: the spans above, and no guessing for an unknown language. */
export const codeLowlight = {
  listLanguages: lowlight.listLanguages,
  registered: lowlight.registered,
  highlight: (language: string, code: string): Tree => tree(highlightCode(language, code)),
  highlightAuto: (code: string): Tree => tree([{ text: code, token: "plain" }]),
};
