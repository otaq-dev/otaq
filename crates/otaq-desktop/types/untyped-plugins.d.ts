// ESLint plugins that ship no TypeScript types (needed because eslint.config.ts is TypeScript too).
declare module "eslint-plugin-jsx-a11y" {
  import type { Linter } from "eslint";
  const plugin: { flatConfigs: { strict: Linter.Config; recommended: Linter.Config } };
  export default plugin;
}
declare module "eslint-plugin-import" {
  import type { ESLint } from "eslint";
  const plugin: ESLint.Plugin;
  export default plugin;
}
