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
			contextLabel: undefined,
			presentation: undefined,
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
			path: "show.expect.active",
			root: "t",
			arguments: ["$empty (result of show.create)"],
		}),
		{
			kind: "expected-outcome",
			description: "Show → active is the created show.",
			surfaces: ["Show files"],
			resultType: undefined,
			expectedOutcome: undefined,
			contextLabel: undefined,
			presentation: undefined,
		},
	);
	assert.deepEqual(
		narrateCall({
			path: "screenshot.application",
			root: "t",
			arguments: ['"bench-application"'],
		}),
		{
			kind: "tool",
			description: 'Capture application screenshot "bench-application".',
			surfaces: ["Generated artifacts"],
			resultType: undefined,
			expectedOutcome: undefined,
			contextLabel: undefined,
			presentation: undefined,
		},
	);
	assert.equal(
		narrateCall({
			path: "builtIn.open",
			root: "t",
			arguments: ["PaneType.Stage"],
		}).description,
		"Built in → open with PaneType.Stage.",
	);
	assert.equal(
		narrateCall({
			path: "expect.toBe",
			root: "expect",
			arguments: ["$first.now (from clock.advanceStep)", '"2020-01-01"'],
		}).description,
		'$first.now (from clock.advanceStep) is "2020-01-01".',
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
	assert.deepEqual(
		narrateCall({
			path: "screen.create",
			root: "t",
			arguments: ['{ name: "Output", bounds: { x: 10, y: 20 } }'],
		}).presentation,
		{
			badges: [{ kind: "screen", label: "Output" }],
			omitConfigurationFields: ["name", "display", "bounds"],
		},
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
	import { scenario } from "../bench/core/scenario";
scenario("EXAMPLE-001", "shows unresolved source", async (t) => {
  await t.app.open();
  await t.show.use(Show.DefaultStage);
  await t.show.create(\`Dynamic \${crypto.randomUUID()}\`);
  await t.unknown.family(value);
  await t.speedGroup.C.expect.synchronizedFrom(t.speedGroup.A.group);
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
	assert.equal(catalog.scenarios[0].preconditions.length, 3);
	assert.equal(catalog.scenarios[0].steps.length, 1);
	assert.equal(catalog.scenarios[0].expectedOutcomes.length, 2);
	assert.equal(
		catalog.scenarios[0].preconditions[0].sourceCode,
		"t.app.open()",
	);
	assert.deepEqual(catalog.scenarios[0].contextLabels, [
		{ kind: "show", label: "Default Stage" },
	]);
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
		catalog.scenarios[0].preconditions[2].description,
		/<generated: UUID>/u,
	);
	assert.equal(
		catalog.scenarios[0].expectedOutcomes[0].description,
		'Speed group → C → synchronized from: "A".',
	);
	assert.deepEqual(
		new Set(catalog.scenarios[0].diagnostics.map(({ code }) => code)),
		new Set(["unresolved-expression", "unknown-narration"]),
	);
	assert.ok(
		catalog.scenarios[0].diagnostics.some(
			(diagnostic) =>
				diagnostic.code === "unresolved-expression" &&
				diagnostic.expression === "value" &&
				diagnostic.relatedCall === "unknown.family",
		),
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
	import { scenario } from "../bench/core/scenario";
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
	const first = await compileSemanticTestCatalog({ root });
	const second = await compileSemanticTestCatalog({ root });
	assert.ok(first.scenarioCount >= marked.length);
	assert.deepEqual(
		new Set(first.scenarios.map((scenario) => scenario.source.file)),
		new Set(
			marked.map((file) => path.relative(root, file).split(path.sep).join("/")),
		),
	);
	assert.equal(
		new Set(
			first.scenarios.map(
				(scenario) => `${scenario.source.file}:${scenario.source.line}`,
			),
		).size,
		first.scenarioCount,
	);
		assert.equal(
			first.scenarios.filter((scenario) => scenario.id === "PROG-002").length,
			8,
		);
	assert.ok(
		first.scenarios.every(
			(scenario) =>
				scenario.migration.status === "migrated-semantic-world" ||
				(scenario.migration.status === "unresolved" &&
					scenario.diagnostics.some((diagnostic) =>
						diagnostic.code.endsWith("-migration-status"),
					)),
		),
	);
	assert.ok(
		first.scenarios.every((scenario) => scenario.expectedOutcomes.length > 0),
	);
	const presetScenario = first.scenarios.find(
		(scenario) => scenario.id === "BENCH-PRESET-001",
	);
	assert.ok(presetScenario);
	assert.ok(
		presetScenario.expectedOutcomes.some((outcome) =>
			outcome.description.includes("PresetFamily.Color"),
		),
	);
	assert.ok(
		presetScenario.diagnostics.every(
			(diagnostic) => !diagnostic.message.includes("family"),
		),
	);
	assert.ok(
		presetScenario.expectedOutcomes.some((outcome) =>
			outcome.description.includes("<observed: t.preset.routeReports.at(-1)>"),
		),
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
				contextLabels: [{ kind: "show", label: "Default Stage" }],
				preconditions: [
					{
						description: "Open ToskLight.",
						source: { line: 2 },
					},
				],
				steps: [
					{
						kind: "step",
						description: "Open the output screen.",
						call: "screen.create",
						order: 0,
						source: { line: 3 },
					},
					{
						kind: "tool",
						description:
							'Create screen <generated: UUID> with { name: "Output", bounds: { x: 10 }, showPlaybacks: true }, <observed: latest report>, and <unresolved: ({ value }) => values.push(value)>.',
						call: "screen.create",
						arguments: [
							'{ name: "Output", bounds: { x: 10 }, showPlaybacks: true }',
						],
						presentation: {
							badges: [{ kind: "screen", label: "Output" }],
							omitConfigurationFields: ["name", "bounds"],
						},
						sourceCode:
							't.screen.create({ name: "Output", bounds: { x: 10 } })',
						order: 1,
						source: { line: 4 },
					},
				],
				expectedOutcomes: [],
				diagnostics: [
					{
						code: "unresolved-expression",
						expression: "first.now",
						message: "Property access first.now depends on a runtime value.",
						source: { line: 3 },
					},
				],
				lastRun: null,
			},
		],
	});
	assert.match(html, /type="search"/u);
	assert.match(html, /id="show-tools" type="checkbox"/u);
	assert.match(html, /id="show-source" type="checkbox"/u);
	assert.match(html, /aria-label="Test suites and test cases"/u);
	assert.match(html, /--header-height/u);
	assert.match(html, /new ResizeObserver\(syncHeaderHeight\)/u);
	assert.match(html, /<span class="tree-type">Suite<\/span>example\.spec\.ts/u);
	assert.match(
		html,
		/<span class="tree-type">Test<\/span><span class="tree-id">HTML-001/u,
	);
	assert.match(html, /application\/json/u);
	assert.match(html, /Preconditions/u);
	assert.match(html, /Scenario contract/u);
	assert.match(html, /kind-tool">Tool/u);
	assert.match(html, /class="tool-row"/u);
	assert.match(html, /class="tool-summary"[^>]*>Tool<\/span>/u);
	assert.match(html, /<table>/u);
	assert.match(html, /context-chip/u);
	assert.match(html, /config-code/u);
	assert.match(html, /json-key/u);
	assert.match(html, /unresolved-token/u);
	assert.match(html, /generated-token/u);
	assert.match(html, /observed-token/u);
	assert.match(html, /diagnostic-code/u);
	assert.match(html, /<pre class="source-code"><code>t\.screen\.create/u);
	const main = /<main id="catalog">([\s\S]+)<\/main>/u.exec(html)?.[1] ?? "";
	assert.match(main, /screen: Output/u);
	assert.match(main, /json-key">showPlaybacks/u);
	assert.doesNotMatch(main, /json-key">(?:name|bounds)/u);
	assert.match(
		main,
		/<span class="unresolved-token">&lt;unresolved: \(\{ value \}\) =&gt; values\.push\(value\)&gt;<\/span>/u,
	);
	assert.match(
		main,
		/<span class="generated-token">&lt;generated: UUID&gt;<\/span>/u,
	);
	assert.match(
		main,
		/<span class="observed-token">&lt;observed: latest report&gt;<\/span>/u,
	);
	assert.match(main, /class="diagnostic-where"[^>]*>Step<\/a>/u);
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
