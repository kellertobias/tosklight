#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { compileSemanticTestCatalog } from "./compiler.mjs";
import { readPlaywrightResults } from "./playwright-results.mjs";
import { renderHtml } from "./render-html.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const options = parseArguments(process.argv.slice(2));
if (options.results && !options.outputDirectory)
	throw new Error(
		"--results requires --output-dir so run-specific data cannot overwrite the deterministic checked catalog",
	);
const outputDirectory = options.outputDirectory
	? path.resolve(options.outputDirectory)
	: path.join(root, "docs/engineering");
const jsonFile = path.join(outputDirectory, "semantic-test-catalog.v1.json");
const htmlFile = path.join(outputDirectory, "semantic-test-catalog.html");
const catalog = await compileSemanticTestCatalog({
	root,
	results: readPlaywrightResults(options.results),
});
const outputs = new Map([
	[jsonFile, `${JSON.stringify(catalog, null, 2)}\n`],
	[htmlFile, renderHtml(catalog)],
]);

if (options.mode === "write") {
	fs.mkdirSync(outputDirectory, { recursive: true });
	for (const [file, content] of outputs) fs.writeFileSync(file, content);
	console.log(
		`Wrote semantic test catalog with ${catalog.scenarioCount} scenarios to ${relative(jsonFile)} and ${relative(htmlFile)}.`,
	);
} else {
	const stale = [];
	for (const [file, content] of outputs) {
		const current = fs.existsSync(file) ? fs.readFileSync(file, "utf8") : "";
		if (current !== content) stale.push(relative(file));
	}
	if (stale.length) {
		const remedy = options.outputDirectory
			? `node tools/semantic-test-docs/cli.mjs --write --output-dir ${JSON.stringify(options.outputDirectory)}${options.results ? ` --results ${JSON.stringify(options.results)}` : ""}`
			: "npm run docs:semantic-tests:write";
		console.error(
			`Semantic test documentation is stale (${stale.join(", ")}); run ${remedy}`,
		);
		process.exitCode = 1;
	} else {
		console.log(
			`Semantic test documentation covers ${catalog.scenarioCount} marked scenarios.`,
		);
	}
}

function parseArguments(args) {
	let mode;
	let results;
	let outputDirectory;
	for (let index = 0; index < args.length; index += 1) {
		const argument = args[index];
		if (argument === "--write" || argument === "--check") {
			if (mode) throw new Error("Choose exactly one of --write or --check");
			mode = argument.slice(2);
		} else if (argument === "--results") {
			results = args[index + 1];
			if (!results) throw new Error("--results requires a Playwright JSON file");
			index += 1;
		} else if (argument === "--output-dir") {
			outputDirectory = args[index + 1];
			if (!outputDirectory) throw new Error("--output-dir requires a directory");
			index += 1;
		} else {
			throw new Error(`Unknown argument: ${argument}`);
		}
	}
	if (!mode)
		throw new Error(
			"Usage: cli.mjs --write|--check [--output-dir directory] [--results report.json]",
		);
	return { mode, results, outputDirectory };
}

function relative(file) {
	return path.relative(root, file).split(path.sep).join("/");
}
