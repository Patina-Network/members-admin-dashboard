import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";

const root = new URL("../../", import.meta.url);
mkdirSync(new URL("frontend/dist/", root), { recursive: true });
const schema = execFileSync(
  "cargo",
  ["run", "--locked", "--quiet", "--bin", "export-openapi"],
  { cwd: root, stdio: ["ignore", "pipe", "inherit"] },
);
writeFileSync(new URL("frontend/openapi.json", root), schema);
