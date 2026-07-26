#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "@babel/parser";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function sourceFiles(root) {
  if (!fs.existsSync(root)) return [];
  const files = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const file = path.join(root, entry.name);
    if (entry.isDirectory()) {
      if (!["dist", "node_modules", "target"].includes(entry.name)) files.push(...sourceFiles(file));
    } else if (
      entry.name.endsWith(".tsx")
      && !entry.name.endsWith(".test.tsx")
      && !entry.name.endsWith(".spec.tsx")
    ) {
      files.push(file);
    }
  }
  return files;
}

export function controlStateLabelWarnings(root = repositoryRoot) {
  const warnings = [];
  for (const file of sourceFiles(path.join(root, "apps"))) {
    const source = parse(fs.readFileSync(file, "utf8"), {
      sourceType: "module",
      plugins: ["jsx", "typescript"],
    });
    function visit(node) {
      if (!node || typeof node !== "object") return;
      if (node.type === "JSXOpeningElement") {
        const component = node.name.type === "JSXIdentifier" ? node.name.name : undefined;
        const names = new Set(node.attributes.flatMap((attribute) =>
          attribute.type === "JSXAttribute" && attribute.name.type === "JSXIdentifier"
            ? [attribute.name.name]
            : []
        ));
        const missing = component === "CheckboxField"
          ? ["stateLabel"].filter((name) => !names.has(name))
          : component === "SwitchField"
            ? ["offLabel", "onLabel"].filter((name) => !names.has(name))
            : [];
        if (missing.length > 0) {
          warnings.push(
            `${path.relative(root, file)}:${node.loc?.start.line ?? 1} ${component} is missing ${missing.join(" and ")}`,
          );
        }
      }
      for (const value of Object.values(node)) {
        if (Array.isArray(value)) {
          for (const child of value) visit(child);
        } else if (value && typeof value === "object" && typeof value.type === "string") {
          visit(value);
        }
      }
    }
    visit(source.program);
  }
  return warnings;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const warnings = controlStateLabelWarnings();
  for (const warning of warnings) console.warn(`control label warning: ${warning}`);
  if (warnings.length === 0) console.log("Checkbox and switch state labels are explicit.");
}
