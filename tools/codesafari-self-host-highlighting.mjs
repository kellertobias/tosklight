#!/usr/bin/env node
// CodeSafari's viewer highlights code with Code Hike's "lighter", whose browser build downloads
// every grammar and theme from https://lighter.codehike.org when a file is opened. That request
// would hand each visitor's IP address to a third party, so the exported site serves the same
// grammar and theme data itself: this script writes them next to the viewer bundle and points the
// bundle's loader at them.
//
// The data comes from the @code-hike/lighter version CodeSafari's viewer bundles (pinned in the
// root package.json). The network loader expects `grammars/<id>.json` to be an array of grammars
// and `themes/<name>.json` to be the theme itself.

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

const REMOTE = "`https://lighter.codehike.org/${";
const LOCAL = "new URL(`lighter/${";
const LOCAL_SUFFIX = "}.json`,import.meta.url)";

const [, , safariDir] = process.argv;
if (!safariDir) {
	console.error("usage: node tools/codesafari-self-host-highlighting.mjs <safari-dir>");
	process.exit(2);
}

const assetsDir = join(safariDir, "assets");
const bundles = readdirSync(assetsDir).filter((name) => name.endsWith(".js"));
const patched = [];
let scopeTable;
for (const name of bundles) {
	const path = join(assetsDir, name);
	const source = readFileSync(path, "utf8");
	if (!source.includes("lighter.codehike.org")) continue;
	scopeTable ??= readScopeTable(name, source);
	// The loader reads `fetch(\`https://lighter.codehike.org/${t}.json\`)`; rewrite exactly that.
	const pattern = /`https:\/\/lighter\.codehike\.org\/\$\{(\w+)\}\.json`/gu;
	const rewritten = source.replace(pattern, (_, variable) => `${LOCAL}${variable}${LOCAL_SUFFIX}`);
	if (rewritten.includes("lighter.codehike.org")) {
		throw new Error(`${name}: lighter.codehike.org is still referenced after rewriting ${REMOTE}…`);
	}
	writeFileSync(path, rewritten);
	patched.push(name);
}
if (!patched.length) {
	console.log("CodeSafari viewer loads no remote highlighting data; nothing to self-host.");
	process.exit(0);
}

// The bundled loader's own table of scope name -> { id, embeddedScopes }. When it fetches
// `grammars/<id>`, it takes every embedded scope from that same file and never asks for them again.
function readScopeTable(name, source) {
	const start = source.indexOf('{"source.abap":{id:');
	if (start === -1) throw new Error(`${name}: cannot find Code Hike's grammar scope table`);
	let depth = 0;
	let end = start;
	for (; end < source.length; end += 1) {
		if (source[end] === "{") depth += 1;
		else if (source[end] === "}" && --depth === 0) break;
	}
	// Parsed as data, never evaluated: only the two property names are unquoted.
	const literal = source.slice(start, end + 1).replace(/([{,])(id|embeddedScopes):/gu, '$1"$2":');
	return JSON.parse(literal);
}

const require = createRequire(import.meta.url);
const lighterDist = dirname(require.resolve("@code-hike/lighter/package.json")) + "/dist";
const load = async (path) => (await import(pathToFileURL(path).href)).default;

const grammars = new Map();
for (const file of readdirSync(join(lighterDist, "grammar")).filter((name) => name.endsWith(".mjs"))) {
	const grammar = await load(join(lighterDist, "grammar", file));
	grammars.set(file.replace(/\.mjs$/u, ""), grammar);
}
const outputDir = join(assetsDir, "lighter");
mkdirSync(join(outputDir, "grammars"), { recursive: true });
mkdirSync(join(outputDir, "themes"), { recursive: true });
const grammarFor = (scope) => {
	const entry = scopeTable[scope];
	const grammar = entry && grammars.get(entry.id);
	if (!grammar) throw new Error(`@code-hike/lighter has no grammar for ${scope}; it must match CodeSafari's bundle`);
	return grammar;
};
for (const [scope, { id, embeddedScopes }] of Object.entries(scopeTable)) {
	const bundle = [grammarFor(scope), ...embeddedScopes.map(grammarFor)];
	writeFileSync(join(outputDir, "grammars", `${id}.json`), JSON.stringify(bundle));
}
const themes = readdirSync(join(lighterDist, "theme")).filter((name) => name.endsWith(".mjs"));
for (const file of themes) {
	const theme = await load(join(lighterDist, "theme", file));
	writeFileSync(join(outputDir, "themes", file.replace(/\.mjs$/u, ".json")), JSON.stringify(theme));
}
console.log(
	`Self-hosted CodeSafari highlighting: ${Object.keys(scopeTable).length} grammars and ${themes.length} themes; ` +
		`patched ${patched.join(", ")}.`,
);
