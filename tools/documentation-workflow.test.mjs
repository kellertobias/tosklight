import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

const source = await readFile(
	process.env.LIGHT_DOCUMENTATION_WORKFLOW_TEST_SOURCE ??
		new URL("../.github/workflows/documentation.yml", import.meta.url),
	"utf8",
);
const concurrency = source.match(/^concurrency:\n([\s\S]*?)(?=^\S)/m)?.[1];
assert.ok(concurrency, "actual workflow concurrency must exist");
const group = concurrency.match(/ {2}group: (.+)(?:\n((?: {4}.+\n)+))?/);
assert.ok(group, "actual workflow concurrency group must exist");
const groupExpression = group[1] === ">-" ? group[2].trim() : group[1];
const jobGuard = source
	.match(
		/ {2}release-metadata:\n[\s\S]*? {4}if: >-\n([\s\S]*?) {4}runs-on:/,
	)?.[1]
	.trim();
assert.ok(jobGuard, "actual job eligibility guard must exist");

// This workflow uses only string equality, short-circuit &&/||, property reads and format.
// Evaluate that actual expression subset (not a duplicate eligibility implementation).
// Hosted concurrency scheduling remains a separately verified integration boundary.
function evaluate(expression, github) {
	return vm.runInNewContext(
		`(${expression})`,
		{
			github,
			format: (template, value) => template.replace("{0}", String(value)),
		},
		{ timeout: 100 },
	);
}
function resolvedGroup(github) {
	if (!groupExpression.startsWith("${{")) return groupExpression;
	return evaluate(groupExpression.slice(3, -2).trim(), github);
}
function completedRun(id, overrides = {}) {
	return {
		event_name: "workflow_run",
		run_id: id,
		event: {
			workflow_run: {
				conclusion: "success",
				event: "push",
				head_branch: "main",
				...overrides,
			},
		},
	};
}

test("eligible publication stays serialized without cancelling the active release", () => {
	const eligible = [
		completedRun(1),
		completedRun(2),
		{ event_name: "schedule", run_id: 3, event: {} },
		{
			event_name: "workflow_dispatch",
			run_id: 4,
			event: { inputs: { force: true } },
		},
		{
			event_name: "workflow_dispatch",
			run_id: 5,
			event: { inputs: { force: false } },
		},
	];
	for (const context of eligible) {
		assert.equal(evaluate(jobGuard, context), true);
		assert.equal(resolvedGroup(context), "nightly-documentation");
	}
	assert.match(concurrency, / {2}cancel-in-progress: false\s/);
});

test("job-skipped CI completions cannot replace either active or pending publication", () => {
	const active = resolvedGroup(completedRun(1));
	const pending = resolvedGroup(completedRun(2));
	assert.equal(
		active,
		pending,
		"eligible publication has one serialized group",
	);
	const ineligible = [
		{ head_branch: "codex/release-osc-subscription-boundary" },
		{ conclusion: "failure" },
		{ conclusion: "cancelled" },
		{ event: "workflow_dispatch" },
		{ event: "pull_request" },
		{ head_branch: undefined },
	];
	const ignoredGroups = new Set();
	for (const [index, fields] of ineligible.entries()) {
		const context = completedRun(100 + index, fields);
		assert.equal(evaluate(jobGuard, context), false);
		const ignored = resolvedGroup(context);
		assert.notEqual(ignored, active);
		assert.notEqual(ignored, pending);
		assert.equal(typeof ignored, "string");
		ignoredGroups.add(ignored);
	}
	assert.equal(ignoredGroups.size, ineligible.length);
});

test("repeated ignored invocations have independent groups even on the same branch", () => {
	const fields = { head_branch: "codex/release-osc-subscription-boundary" };
	assert.notEqual(
		resolvedGroup(completedRun(8, fields)),
		resolvedGroup(completedRun(9, fields)),
	);
});

test("handbook consumes the complete fresh gallery before rendering", () => {
	const manual = source.match(/^  manual:\n([\s\S]*?)(?=^  [\w-]+:)/m)?.[1];
	assert.ok(manual);
	assert.match(
		manual,
		/needs: \[release-metadata, release-performance, help-screenshots\]/,
	);
	assert.ok(
		manual.indexOf("name: help-screenshots") <
			manual.indexOf("run: npm run manual"),
	);
	for (const name of ["help-screenshots-storybook", "help-screenshots-live"]) {
		assert.match(
			source,
			new RegExp(`name: ${name}\\n          path: \\.artifacts/tmp/${name}`),
		);
	}
});

test("current manual publication verifies source-bound captures and preserves the site", async () => {
	const workflow = await readFile(
		new URL("../.github/workflows/manual-pages.yml", import.meta.url),
		"utf8",
	);
	assert.ok(
		workflow.indexOf("manual-captures.py restore") <
			workflow.indexOf("run: npm run manual"),
	);
	assert.match(workflow, /sha256sum --check/);
	assert.match(workflow, /group: nightly-documentation/);
	assert.match(workflow, /manual-publication\.json/);
	assert.match(
		workflow,
		/cp -R \.artifacts\/generated\/manual\/html\/tosklight-manual \.artifacts\/generated\/pages\/manual/,
	);
	assert.doesNotMatch(workflow, /rm -rf \.artifacts\/generated\/pages\n/);
});
