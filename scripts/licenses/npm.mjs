// The npm part of THIRD-PARTY-LICENSES.md (d0071, T-0245), printed to stdout; scripts/licenses.sh appends it.
//
// Lists the page's production packages: the `dependencies` of crates/otaq-desktop/package.json and what they pull
// in, as package-lock.json records them (every package not marked `dev`). Vite bundles exactly these into the page the
// app ships. The development tools (Vite, TypeScript, ESLint, Vitest, the Tauri CLI) only build and check the page and
// ship nothing, so they're left out. The license texts come from the installed packages (`npm ci` first); the order
// is the lock file's names, sorted, so every run gives the same text.
//
//   node scripts/licenses/npm.mjs <folder with package.json and node_modules>

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2];
const lock = JSON.parse(readFileSync(join(dir, "package-lock.json"), "utf8"));

const fail = (message) => {
  console.error(`licenses.sh: ${message}`);
  process.exit(1);
};

const packages = Object.entries(lock.packages)
  .filter(([path, entry]) => path !== "" && !entry.dev && !entry.devOptional)
  .map(([path, entry]) => {
    const name = path.slice(path.lastIndexOf("node_modules/") + "node_modules/".length);
    const folder = join(dir, path);
    let installed;
    try {
      installed = JSON.parse(readFileSync(join(folder, "package.json"), "utf8"));
    } catch {
      fail(`${name} isn't installed in ${dir}/node_modules; run npm ci there first`);
    }
    if (installed.version !== entry.version) {
      fail(`${name} ${installed.version} is installed, the lock file wants ${entry.version}; run npm ci first`);
    }
    // LICENSE, LICENSE-MIT, LICENSE-APACHE-2.0, COPYING, NOTICE…; not the SPDX metadata file.
    const files = readdirSync(folder)
      .filter((file) => /^(licen[cs]e|copying|notice)/i.test(file) && !file.endsWith(".spdx"))
      .sort();
    if (files.length === 0) fail(`${name} ships no license file`);
    const texts = files.map((file) => readFileSync(join(folder, file), "utf8").trimEnd());
    const repository = installed.repository?.url ?? installed.repository ?? `https://www.npmjs.com/package/${name}`;
    const source = repository.replace(/^git\+/, "").replace(/\.git$/, "");
    return { name, version: entry.version, license: entry.license ?? installed.license, source, texts };
  })
  .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));

const out = [
  "## npm packages",
  "",
  "The page's production packages (`crates/otaq-desktop/package.json` and what they pull in), bundled into the",
  "app. The tools that only build and check the page (Vite, TypeScript, ESLint, Vitest, the Tauri CLI) don't ship and",
  "aren't listed.",
  "",
  ...packages.map((p) => `- ${p.name} ${p.version} (${p.license})`),
  "",
];
for (const p of packages) {
  out.push(`### ${p.name} ${p.version}`, "", `${p.license}, from ${p.source}`, "");
  for (const text of p.texts) out.push("```text", text, "```", "");
}
process.stdout.write(out.join("\n"));
