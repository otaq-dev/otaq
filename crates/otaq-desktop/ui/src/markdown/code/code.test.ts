// The code block's plain parts (T-0254): the fence that fits the code (Q48), the picker's list and filter, and the
// mapping of highlight.js onto the Files tab's tokens.

import { describe, expect, test } from "vitest";

import { codeBlockMarkdown, fenceFor } from "./fence";
import { codeLowlight, highlightCode, tokenOf } from "./highlight";
import { CODE_LANGUAGES, filterLanguages, isCurrentLanguage, languageLabel } from "./languages";

describe("the fence fits the code (Q48)", () => {
  test.each([
    ["no backticks", "const a = 1;", "```"],
    ["inline backticks", "let s = `a`;", "```"],
    ["a ``` line", "```ts\nconst a = 1;\n```", "````"],
    ["a ```` line", "````\n```\ninner\n```\n````", "`````"],
    ["an indented ``` line", "   ```", "````"],
    ["``` in the middle of a line closes nothing", 'echo "```"', "```"],
    ["four spaces make it code, not a fence", "    ````", "```"],
    ["empty", "", "```"],
  ])("%s", (_name, code, fence) => {
    expect(fenceFor(code)).toBe(fence);
  });

  test("the block: fence and language, code, fence", () => {
    expect(codeBlockMarkdown("ts", "const a = 1;")).toBe("```ts\nconst a = 1;\n```");
    expect(codeBlockMarkdown(null, "```\nx\n```")).toBe("````\n```\nx\n```\n````");
    expect(codeBlockMarkdown("md", "")).toBe("```md\n\n```");
  });
});

describe("the language picker's list", () => {
  test("Plain text, then the eight languages, in the drawn order", () => {
    expect(CODE_LANGUAGES.map((l) => l.label)).toEqual([
      "Plain text",
      "bash",
      "diff",
      "json",
      "rust",
      "ts",
      "tsx",
      "yaml",
      "toml",
    ]);
    expect(CODE_LANGUAGES[0]?.id).toBeNull();
  });

  test.each([
    ["", 9],
    ["ts", 2],
    ["TS", 2],
    [" y ", 1],
    ["plain", 1],
    ["none", 1],
    ["python", 0],
  ])("filter %j shows %i", (query, count) => {
    expect(filterLanguages(query)).toHaveLength(count);
  });

  test("the label and the current row", () => {
    expect(languageLabel("ts")).toBe("ts");
    expect(languageLabel(null)).toBe("Plain text");
    expect(languageLabel("")).toBe("Plain text");
    expect(languageLabel("python")).toBe("python");
    const [plain, bash] = CODE_LANGUAGES;
    if (!plain || !bash) throw new Error("no rows");
    expect(isCurrentLanguage(plain, null)).toBe(true);
    expect(isCurrentLanguage(plain, "")).toBe(true);
    expect(isCurrentLanguage(bash, "bash")).toBe(true);
    expect(isCurrentLanguage(bash, null)).toBe(false);
  });
});

describe("code colours are the Files tab's tokens", () => {
  test("highlight.js classes", () => {
    expect(tokenOf(["hljs-keyword"], "ts")).toBe("keyword");
    expect(tokenOf(["hljs-string"], "ts")).toBe("string");
    expect(tokenOf(["hljs-number"], "ts")).toBe("number");
    expect(tokenOf(["hljs-literal"], "json")).toBe("number");
    expect(tokenOf(["hljs-comment"], "rust")).toBe("comment");
    expect(tokenOf(["hljs-title", "function_"], "rust")).toBe("function");
    expect(tokenOf(["hljs-title", "class_"], "ts")).toBe("type");
    expect(tokenOf(["hljs-type"], "rust")).toBe("type");
    expect(tokenOf(["hljs-built_in"], "ts")).toBe("type");
    expect(tokenOf(["hljs-built_in"], "bash")).toBe("function");
    expect(tokenOf(["hljs-attr"], "json")).toBeNull();
  });

  test("the example's ts line", () => {
    const spans = highlightCode("ts", 'const results = await places.autocomplete(query, { country: "TR" });');
    expect(spans.map((s) => s.text).join("")).toBe(
      'const results = await places.autocomplete(query, { country: "TR" });',
    );
    expect(spans.filter((s) => s.token === "keyword").map((s) => s.text)).toEqual(["const", "await"]);
    expect(spans.find((s) => s.token === "string")?.text).toBe('"TR"');
  });

  test.each(["bash", "diff", "json", "rust", "ts", "tsx", "yaml", "toml"])("%s is loaded", (language) => {
    expect(codeLowlight.registered(language)).toBe(true);
  });

  test("a language the picker doesn't offer stays plain", () => {
    expect(codeLowlight.registered("python")).toBe(false);
    expect(highlightCode("python", "def f(): pass")).toEqual([{ text: "def f(): pass", token: "plain" }]);
  });
});
