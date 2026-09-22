#!/usr/bin/env node

import { readdir } from "node:fs/promises";
import { join } from "node:path";

const root = join(process.cwd(), "openspec", "changes");
const valid = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const entries = await readdir(root, { withFileTypes: true });
const invalid = entries
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .filter((name) => name !== "archive" && !name.startsWith(".") && !valid.test(name));

if (invalid.length > 0) {
  console.error("Invalid active OpenSpec change names:");
  for (const name of invalid) console.error(`- ${name}`);
  console.error("Names must match ^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$");
  process.exit(1);
}

console.log("OK: active OpenSpec change names are letter-first kebab-case");
