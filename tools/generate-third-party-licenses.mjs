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
  <meta name="theme-color" content="#07090d" />
  <title>ToskLight third-party license notices</title>
  <link rel="icon" href="icon.png" type="image/png" />
  <link rel="stylesheet" href="site.css" />
</head>
<body class="legal-page">
  <nav class="topbar shell"><a class="wordmark" href="./"><img src="icon.png" alt="" /><span>ToskLight</span></a><div class="nav-links"><a href="downloads/">Downloads</a><a href="performance/">Development</a><a class="nav-cta" href="./">Back to the suite</a></div></nav>
  <main class="document-page shell">
    <header class="document-hero"><p class="eyebrow">Open-source software</p><h1>Third-party licenses.</h1><p>ToskLight stands on excellent open-source work. Every package remains available under its own declared license.</p></header>
    <section class="license-summary"><p>ToskLight itself is distributed under the <a href="license/">ToskLight Community License</a>. This notice is generated from the resolved production dependency graphs: ${npm.length} npm packages and ${rust.length} Rust packages. It excludes development-only dependencies.</p><p>Generated ${escape(generatedAt)}.</p></section>
    <div class="license-table-scroll"><table class="license-table">
      <thead><tr><th>Ecology</th><th>Package</th><th>Version</th><th>Declared license</th><th>Project source</th></tr></thead>
      <tbody>
${table([...npm, ...rust])}
      </tbody>
    </table></div>
  </main>
  <footer><div class="shell download-footer"><p><a href="license/">ToskLight Community License</a> · <a href="imprint/">Imprint &amp; Privacy</a></p><a href="./">← Back to ToskLight</a></div></footer>
</body>
</html>
`;
writeFileSync(output, document);
console.log(`Generated ${relative(root, output)} with ${npm.length + rust.length} license notices`);
