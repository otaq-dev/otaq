import react from "@astrojs/react";
import { defineConfig } from "astro/config";

// Static output at https://otaq.io with no base path (d0101, d0104).
export default defineConfig({
  site: "https://otaq.io",
  output: "static",
  integrations: [react()],
});
