#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { basename, relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const escape = (value) =>
	String(value ?? "")
		.replaceAll("&", "&amp;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll('"', "&quot;");

function nodePackages() {
	const lock = JSON.parse(readFileSync(resolve(root, "package-lock.json"), "utf8"));
	return Object.entries(lock.packages ?? {})
		.filter(([path, metadata]) => path.startsWith("node_modules/") && !metadata.dev)
		.map(([path, metadata]) => ({
			ecosystem: "npm",
			name: metadata.name ?? basename(path),
			version: metadata.version ?? "unknown",
			license: metadata.license ?? "License not declared in package-lock",
			source: metadata.resolved ?? metadata.repository?.url ?? "",
		}))
		.sort((left, right) => left.name.localeCompare(right.name) || left.version.localeCompare(right.version));
}

function rustPackages() {
	const metadata = JSON.parse(
		execFileSync("cargo", ["metadata", "--locked", "--format-version=1"], {
			cwd: root,
			encoding: "utf8",
			maxBuffer: 32 * 1024 * 1024,
		}),
	);
	const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
	const workspace = new Set(metadata.workspace_members);
	const resolved = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
	const pending = [...workspace];
	const reached = new Set();
	while (pending.length) {
		const id = pending.pop();
		if (reached.has(id)) continue;
		reached.add(id);
		for (const dependency of resolved.get(id)?.deps ?? []) {
			if (dependency.dep_kinds.some(({ kind }) => kind !== "dev")) pending.push(dependency.pkg);
		}
	}
	return [...reached]
		.filter((id) => !workspace.has(id))
		.map((id) => packages.get(id))
		.filter(Boolean)
		.map((pkg) => ({
			ecosystem: "Rust",
			name: pkg.name,
			version: pkg.version,
			license: pkg.license ?? "License not declared in Cargo metadata",
			source: pkg.repository ?? pkg.homepage ?? "",
		}))
		.sort((left, right) => left.name.localeCompare(right.name) || left.version.localeCompare(right.version));
}

function table(rows) {
	return rows
		.map(
			({ ecosystem, name, version, license, source }) => `<tr>
  <td>${escape(ecosystem)}</td>
  <td>${escape(name)}</td>
  <td>${escape(version)}</td>
  <td>${escape(license)}</td>
  <td>${source ? `<a href="${escape(source)}">Source</a>` : "—"}</td>
</tr>`,
		)
		.join("\n");
}

const output = process.argv[2];
if (!output) throw new Error("Usage: generate-third-party-licenses.mjs OUTPUT.html");

const npm = nodePackages();
const rust = rustPackages();
const generatedAt = new Date().toISOString();
const document = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>ToskLight third-party license notices</title>
  <style>
    body { max-width: 90rem; margin: 2rem auto; padding: 0 1.25rem; color: #17202a; font: 16px/1.5 system-ui, sans-serif; }
    table { width: 100%; border-collapse: collapse; }
    th, td { padding: .5rem; text-align: left; vertical-align: top; border-bottom: 1px solid #dce1e7; }
    th { background: #f4f6f8; position: sticky; top: 0; }
    code { font-size: .9em; }
  </style>
</head>
<body>
  <h1>Third-party license notices</h1>
  <p>ToskLight itself is distributed under the <a href="license.txt">ToskLight Community License</a>. The packages below remain under their own licenses.</p>
  <p>This notice is generated from the resolved production dependency graphs: ${npm.length} npm packages and ${rust.length} Rust packages. It excludes development-only dependencies. Generated ${escape(generatedAt)}.</p>
  <table>
    <thead><tr><th>Ecology</th><th>Package</th><th>Version</th><th>Declared license</th><th>Project source</th></tr></thead>
    <tbody>
${table([...npm, ...rust])}
    </tbody>
  </table>
</body>
</html>
`;
writeFileSync(output, document);
console.log(`Generated ${relative(root, output)} with ${npm.length + rust.length} license notices`);
