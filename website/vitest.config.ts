/// <reference types="vitest/config" />
import { getViteConfig } from "astro/config";

// Unit tests next to the code they test (`src/**/*.test.ts`, `.test.tsx`). Node by default; a test that needs a
// DOM picks one in its own file with a `// @vitest-environment …` comment.
export default getViteConfig({
  test: {
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    environment: "node",
    passWithNoTests: true,
  },
});
