import fs from "node:fs";
import path from "node:path";

export const semanticWorldMarker = "@bench-semantic-world";

const forbidden = [
	["Playwright Page or Locator", /import\s+type\s*\{[^}]*\b(?:Page|Locator)\b[^}]*\}\s+from\s+["']@playwright\/test["']/u],
	["the low-level ApiDriver", /\bApiDriver\b/u],
	["raw HTTP", /\b(?:fetch|request)\s*\(/u],
	["application reducer actions", /(?:dispatch|send)\s*\(\s*\{\s*type\s*:/u],
	["application CSS selectors", /\.(?:locator|querySelector|querySelectorAll)\s*\(/u],
	["fixture UUID resolution", /\bfixtureIds?\b/u],
	["coordinate pointer interaction", /\.(?:click|move)\s*\(\s*\d+\s*,/u],
	["physical encoder slots", /\bencoderSlot\b|\bslotIndex\b/u],
	["raw mutable show objects", /\b(?:seedShowObject|putObject|deleteSeededShowObject)\b/u],
];

export function scanSemanticWorldSource(name, source) {
	if (!source.includes(semanticWorldMarker)) return [];
	return forbidden
		.filter(([, pattern]) => pattern.test(source))
		.map(([description]) =>
			`${name} is marked ${semanticWorldMarker} but imports or uses ${description}`,
		);
}

export function scanSemanticWorldFiles(root) {
	const tests = path.join(root, "tests");
	return fs.readdirSync(tests, { withFileTypes: true })
		.filter((entry) => entry.isFile() && entry.name.endsWith(".spec.ts"))
		.flatMap((entry) => {
			const file = path.join(tests, entry.name);
			return scanSemanticWorldSource(
				`tests/${entry.name}`,
				fs.readFileSync(file, "utf8"),
			);
		});
}
