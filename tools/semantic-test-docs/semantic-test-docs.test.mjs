import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
	compileSemanticTestCatalog,
	discoverMarkedSpecs,
} from "./compiler.mjs";
import { narrateCall } from "./narration-catalog.mjs";
import { readPlaywrightResults, resultFor } from "./playwright-results.mjs";
import { renderHtml } from "./render-html.mjs";

const root = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../..",
);

test("central narration separates actions, outcomes, surfaces, and returned handles", () => {
	assert.deepEqual(
		narrateCall({
			path: "command.execute",
			root: "t",
			arguments: ['"GROUP 1 AT 50"'],
		}),
		{
			kind: "step",
			description: 'Execute the desk command "GROUP 1 AT 50".',
			surfaces: ["Command line"],
			resultType: undefined,
			expectedOutcome: undefined,
		},
	);
	assert.equal(
		narrateCall({
			path: "group.expect.fixtures",
			root: "t",
			arguments: ["4", "1", "2"],
		}).kind,
		"expected-outcome",
	);
	assert.deepEqual(
		narrateCall({
			path: "selection.fixtures.via.osc.item",
			root: "t",
			arguments: ["1"],
		}).surfaces,
		["OSC", "Selection"],
	);
	assert.equal(
		narrateCall({
			path: "desktop.configure",
			root: "t",
			arguments: ['"Operator"'],
		}).resultType,
		"desktopBuilder",
	);
});

test("AST compiler preserves dynamic expressions and unknown helpers as diagnostics", async () => {
	const fixture = fs.mkdtempSync(
		path.join(os.tmpdir(), "light-semantic-docs-"),
	);
	const tests = path.join(fixture, "tests");
	const docs = path.join(fixture, "docs/engineering");
	fs.mkdirSync(tests, { recursive: true });
	fs.mkdirSync(docs, { recursive: true });
	const sourceFile = path.join(tests, "example.spec.ts");
	fs.writeFileSync(
		sourceFile,
		`// @bench-semantic-world
	import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
scenario("EXAMPLE-001", "shows unresolved source", async (t) => {
  await t.app.open();
  await t.show.create(\`Dynamic \${crypto.randomUUID()}\`);
  await t.unknown.family(value);
  await t.show.expect.dirty(false);
});
`,
	);
	const inventory = path.join(docs, "test-bench-migration-inventory.md");
	fs.writeFileSync(
		inventory,
		`| Source | Scenario and intent | Contract | Surfaces | Helper family | Artifacts | Constraint | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| example.spec.ts | EXAMPLE-001 @bench @ui › shows unresolved source | docs/testing/example.md | @bench @ui | example | standard failure evidence | parallel | migrated-semantic-world |
`,
	);

	const catalog = await compileSemanticTestCatalog({
		root: fixture,
		inventoryFile: inventory,
		sourceFiles: [sourceFile],
	});
	assert.equal(catalog.scenarioCount, 1);
	assert.equal(catalog.scenarios[0].steps.length, 3);
	assert.equal(catalog.scenarios[0].expectedOutcomes.length, 1);
	assert.equal(
		catalog.scenarios[0].migration.status,
		"migrated-semantic-world",
	);
	assert.ok(
		catalog.scenarios[0].testedSurfaces.every(
			(surface) => !surface.startsWith("@"),
		),
	);
	assert.match(
		catalog.scenarios[0].steps[1].description,
		/<unresolved: `Dynamic/u,
	);
	assert.deepEqual(
		new Set(catalog.scenarios[0].diagnostics.map(({ code }) => code)),
		new Set(["unresolved-expression", "unknown-narration"]),
	);
});

test("ambiguous or missing migration rows stay unresolved and branches stay visible", async () => {
	const fixture = fs.mkdtempSync(
		path.join(os.tmpdir(), "light-semantic-status-"),
	);
	const tests = path.join(fixture, "tests");
	const docs = path.join(fixture, "docs/engineering");
	fs.mkdirSync(tests, { recursive: true });
	fs.mkdirSync(docs, { recursive: true });
	const sourceFile = path.join(tests, "status.spec.ts");
	fs.writeFileSync(
		sourceFile,
		`// @bench-semantic-world
	import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
scenario("STATUS-001", "ambiguous row", async (t) => {
  if (runtime.enabled) await t.group.stroe(1);
  else await t.app.open();
  expect(runtime.value).toBe(1);
});
scenario("STATUS-002", "missing row", async (t) => {
  await t.app.open();
  await t.app.expect.ready();
});
`,
	);
	const inventory = path.join(docs, "test-bench-migration-inventory.md");
	const row =
		"| status.spec.ts | STATUS-001 @bench @ui › ambiguous row | docs/testing/status.md | @bench @ui | status | standard failure evidence | parallel | migrated-semantic-world |";
	fs.writeFileSync(
		inventory,
		`| Source | Scenario and intent | Contract | Surfaces | Helper family | Artifacts | Constraint | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
${row}
${row}
`,
	);
	const catalog = await compileSemanticTestCatalog({
		root: fixture,
		inventoryFile: inventory,
		sourceFiles: [sourceFile],
	});
	assert.deepEqual(
		catalog.scenarios.map((scenario) => scenario.migration.status),
		["unresolved", "unresolved"],
	);
	assert.ok(
		catalog.scenarios[0].diagnostics.some(
			(diagnostic) => diagnostic.code === "ambiguous-migration-status",
		),
	);
	assert.ok(
		catalog.scenarios[1].diagnostics.some(
			(diagnostic) => diagnostic.code === "missing-migration-status",
		),
	);
	assert.ok(
		catalog.scenarios[0].diagnostics.some(
			(diagnostic) => diagnostic.code === "static-control-flow",
		),
	);
	assert.ok(
		catalog.scenarios[0].diagnostics.some(
			(diagnostic) => diagnostic.code === "unknown-narration",
		),
	);
	assert.ok(
		catalog.scenarios[0].diagnostics.some(
			(diagnostic) =>
				diagnostic.code === "unresolved-expression" &&
				diagnostic.message.includes("runtime.value"),
		),
	);
});

test("repository compiler finds every marked scenario once and stays deterministic", async () => {
	const marked = discoverMarkedSpecs(path.join(root, "tests"));
	assert.equal(marked.length, 10);
	const first = await compileSemanticTestCatalog({ root });
	const second = await compileSemanticTestCatalog({ root });
	assert.equal(first.scenarioCount, 27);
	assert.equal(
		new Set(
			first.scenarios.map(
				(scenario) => `${scenario.source.file}:${scenario.source.line}`,
			),
		).size,
		27,
	);
	assert.equal(
		first.scenarios.filter((scenario) => scenario.id === "PROG-002").length,
		2,
	);
	assert.ok(
		first.scenarios.every(
			(scenario) => scenario.migration.status === "migrated-semantic-world",
		),
	);
	assert.ok(
		first.scenarios.every((scenario) => scenario.expectedOutcomes.length > 0),
	);
	assert.ok(
		first.scenarios.every((scenario) =>
			scenario.diagnostics.every(
				(diagnostic) => diagnostic.code !== "unknown-narration",
			),
		),
	);
	assert.deepEqual(second, first);
});

test("Playwright JSON merges only into separate last-run metadata with aggregate precedence", () => {
	const fixture = fs.mkdtempSync(path.join(os.tmpdir(), "light-results-"));
	const report = path.join(fixture, "report.json");
	fs.writeFileSync(
		report,
		JSON.stringify({
			suites: [
				{
					file: "/repo/tests/example.spec.ts",
					specs: [
						{
							title: "EXAMPLE-001 @bench @ui › last run",
							tests: [
								{
									status: "skipped",
									results: [{ status: "skipped", duration: 0 }],
								},
								{
									status: "expected",
									results: [{ status: "passed", duration: 12 }],
								},
							],
						},
						{
							title: "EXAMPLE-002 @bench @ui › flaky run",
							tests: [
								{ status: "skipped", results: [{ status: "skipped" }] },
								{
									status: "flaky",
									results: [{ status: "passed", duration: 4 }],
								},
							],
						},
						{
							title: "EXAMPLE-003 @bench @ui › failed run",
							tests: [
								{ status: "expected", results: [{ status: "passed" }] },
								{
									status: "unexpected",
									results: [{ status: "failed", duration: 3 }],
								},
							],
						},
					],
				},
			],
		}),
	);
	const results = readPlaywrightResults(report);
	assert.deepEqual(
		resultFor(results, "tests/example.spec.ts", "EXAMPLE-001", "last run"),
		{ status: "passed", durationMs: 12, attempts: 2 },
	);
	assert.equal(
		resultFor(results, "tests/example.spec.ts", "EXAMPLE-002", "flaky run")
			.status,
		"flaky",
	);
	assert.equal(
		resultFor(results, "tests/example.spec.ts", "EXAMPLE-003", "failed run")
			.status,
		"failed",
	);
	assert.equal(
		resultFor(
			results,
			"tests/example.spec.ts",
			"EXAMPLE-001",
			"expected outcome",
		),
		null,
	);
});

test("HTML is self-contained, searchable, and safely embeds catalog text", () => {
	const html = renderHtml({
		schemaVersion: 1,
		scenarios: [
			{
				id: "HTML-001",
				title: "</script><script>alert(1)</script>",
				source: { file: "tests/example.spec.ts", line: 1 },
				migration: { status: "migrated-semantic-world" },
				testedSurfaces: [],
				steps: [],
				expectedOutcomes: [],
				diagnostics: [],
				lastRun: null,
			},
		],
	});
	assert.match(html, /type="search"/u);
	assert.match(html, /application\/json/u);
	assert.doesNotMatch(html, /<\/script><script>alert/u);
	assert.doesNotMatch(html, /<link|src="https?:/u);
});

test("CLI check rejects stale alternate outputs and accepts a subsequent write", () => {
	const outputDirectory = fs.mkdtempSync(
		path.join(os.tmpdir(), "light-doc-check-"),
	);
	const cli = path.join(root, "tools/semantic-test-docs/cli.mjs");
	const args = ["--output-dir", outputDirectory];
	const stale = spawnSync(process.execPath, [cli, "--check", ...args], {
		cwd: root,
		encoding: "utf8",
	});
	assert.equal(stale.status, 1);
	assert.match(stale.stderr, /is stale/u);
	assert.match(stale.stderr, /--write --output-dir/u);

	const write = spawnSync(process.execPath, [cli, "--write", ...args], {
		cwd: root,
		encoding: "utf8",
	});
	assert.equal(write.status, 0, write.stderr);
	const check = spawnSync(process.execPath, [cli, "--check", ...args], {
		cwd: root,
		encoding: "utf8",
	});
	assert.equal(check.status, 0, check.stderr);
});

test("CLI refuses run-specific results in the canonical documentation directory", () => {
	const fixture = fs.mkdtempSync(path.join(os.tmpdir(), "light-doc-results-"));
	const report = path.join(fixture, "report.json");
	fs.writeFileSync(report, JSON.stringify({ suites: [] }));
	const cli = path.join(root, "tools/semantic-test-docs/cli.mjs");
	const result = spawnSync(
		process.execPath,
		[
			cli,
			"--write",
			"--results",
			report,
			"--output-dir",
			path.join(root, "docs/engineering"),
		],
		{ cwd: root, encoding: "utf8" },
	);
	assert.notEqual(result.status, 0);
	assert.match(result.stderr, /must be outside docs\/engineering/u);
});
