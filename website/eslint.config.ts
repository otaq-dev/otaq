// eslint-disable-next-line @typescript-eslint/triple-slash-reference -- ambient types for two untyped plugins; the linter's default project for config files doesn't load "include"
/// <reference path="./types/untyped-plugins.d.ts" />
import comments from "@eslint-community/eslint-plugin-eslint-comments";
import js from "@eslint/js";
import astro from "eslint-plugin-astro";
import importPlugin from "eslint-plugin-import";
import jsxA11y from "eslint-plugin-jsx-a11y";
import reactHooks from "eslint-plugin-react-hooks";
import type { Linter } from "eslint";
import { defineConfig } from "eslint/config";
import globals from "globals";
import tseslint from "typescript-eslint";

// The app's rules (crates/otaq-desktop/eslint.config.ts, SDD UIT-08), plus Astro's for `.astro` files.
export default defineConfig(
  {
    ignores: ["dist/**", ".astro/**", "node_modules/**", "types/**"],
  },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  reactHooks.configs.flat.recommended,
  astro.configs["flat/recommended"],
  {
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        projectService: { allowDefaultProject: ["*.config.ts"], defaultProject: "tsconfig.node.json" },
        tsconfigRootDir: import.meta.dirname,
        extraFileExtensions: [".astro"],
      },
    },
    linterOptions: { reportUnusedDisableDirectives: "error" },
    plugins: { import: importPlugin, "eslint-comments": comments },
    settings: { "import/resolver": { node: true } },
    rules: {
      "@typescript-eslint/no-floating-promises": "error",
      "@typescript-eslint/no-misused-promises": "error",
      "@typescript-eslint/switch-exhaustiveness-check": [
        "error",
        { considerDefaultExhaustiveForUnions: false, requireDefaultForNonUnion: true },
      ],
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "error",
      "import/no-extraneous-dependencies": [
        "error",
        {
          devDependencies: ["*.config.ts", "**/*.test.ts", "**/*.test.tsx"],
          includeTypes: true,
        },
      ],
      // A disabled rule needs a written reason (d0075 point 5): `-- reason` after the directive.
      "eslint-comments/require-description": ["error", { ignore: [] }],
      "eslint-comments/no-unlimited-disable": "error",
    },
  },
  {
    files: ["src/**/*.tsx"],
    ...jsxA11y.flatConfigs.strict,
  },
  {
    // The same jsx-a11y strict rules for `.astro` markup (as `astro/jsx-a11y/*`). Only the rules: the preset also
    // registers its own copy of the `jsx-a11y` plugin, which clashes with the one above.
    files: ["**/*.astro"],
    rules: Object.fromEntries(
      astro.configs["flat/jsx-a11y-strict"].flatMap((config: Linter.Config) =>
        Object.entries(config.rules ?? {}),
      ),
    ),
  },
  {
    // astro-eslint-parser has no projectService; it finds tsconfig.json with `project` instead.
    files: ["**/*.astro"],
    languageOptions: { parserOptions: { projectService: false, project: "./tsconfig.json" } },
  },
  {
    files: ["*.config.ts"],
    languageOptions: { globals: globals.node },
  },
);
