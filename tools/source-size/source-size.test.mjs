import assert from "node:assert/strict";
import test from "node:test";
import { isSourcePath, isTestSource } from "./config.mjs";
import { scanJavaScriptFunctions } from "./javascript.mjs";
import { scanPythonFunctions } from "./python.mjs";
import {
	baselineFor,
	evaluateRatcheting,
	serializeBaseline,
} from "./ratchet.mjs";
import { scanRustFunctions } from "./rust.mjs";
import { logicalLineCount } from "./shared.mjs";
import { scanShellFunctions } from "./shell.mjs";

test("logical lines count a final unterminated line", () => {
	assert.equal(logicalLineCount(""), 0);
	assert.equal(logicalLineCount("one"), 1);
	assert.equal(logicalLineCount("one\ntwo\n"), 2);
});

test("only code files in production source roots are scanned", () => {
	for (const repositoryPath of [
		"apps/light-desktop/src/App.tsx",
		"apps/light-desktop/src/api.ts",
		"apps/light-desktop/scripts/setup.js",
		"crates/light/adapters/headless/src/main.rs",
		"packages/tools/check.py",
	]) {
		assert.equal(isSourcePath(repositoryPath), true, repositoryPath);
	}

	for (const repositoryPath of [
		"Cargo.lock",
		"apps/light-desktop/package.json",
		"apps/light-desktop/README.md",
		"apps/light-desktop/src/style.css",
		"apps/light-desktop/assets/config.js",
		"apps/light-desktop/artifacts/report.ts",
		"apps/light-desktop/docs/example.tsx",
		"apps/light-desktop/experiments/panel.tsx",
		"docs/generated.ts",
		"experiments/dynamics-editor/app.js",
		"tests/large.spec.ts",
		"tools/check-source-size.mjs",
	]) {
		assert.equal(isSourcePath(repositoryPath), false, repositoryPath);
	}
});

test("test sources are identified without exempting them from goal reporting", () => {
	assert.equal(isTestSource("tests/large.spec.ts"), true);
	assert.equal(
		isTestSource("crates/light/adapters/headless/tests/runtime.rs"),
		true,
	);
	assert.equal(isTestSource("tests/bench/fixture.ts"), true);
	assert.equal(isTestSource("apps/light-desktop/src/Panel.test.tsx"), true);
	assert.equal(
		isTestSource("apps/light-desktop/src/__tests__/Panel.tsx"),
		true,
	);
	assert.equal(isTestSource("apps/light-desktop/src/Panel.tsx"), false);
	assert.equal(
		isTestSource("crates/light/adapters/headless/src/runtime.rs"),
		false,
	);
});

test("Rust test modules and Storybook stories are test sources", () => {
	assert.equal(
		isTestSource("crates/light/src/playback/service_tests.rs"),
		true,
	);
	assert.equal(
		isTestSource(
			"crates/light/src/programming/live_state_tests/values_actions.rs",
		),
		true,
	);
	assert.equal(
		isTestSource("crates/shared/core/src/attributes/tests.rs"),
		true,
	);
	assert.equal(
		isTestSource("apps/light-desktop/src/windows/Stage.stories.tsx"),
		true,
	);
});

test("Rust scanner ignores literals and nested comments", () => {
	const source = `const TEXT: &str = r#"fn fake() { }"#;
/* outer /* fn hidden() {} */ still hidden */
fn real(
    value: usize,
) -> usize {
    let brace = "}";
    value + 1
}
`;
	const findings = scanRustFunctions("sample.rs", source);
	assert.deepEqual(
		findings.map(({ display, lines }) => [display, lines]),
		[["real (function)", 6]],
	);
});

test("TypeScript scanner finds callable bodies without TSX false positives", () => {
	const source = `const icon = "💡";
const text = \`function fake() {} \${notInterpolation}\`;
const pattern = /function fake\\(\\) \\{\\}/;
export function Panel(value: boolean) {
  const calculated = useMemo(
    () => ({
      nested: { value },
      render: () => <span>{value ? "yes" : "no"}</span>,
    }),
    [value],
  );
  return <section>{value && calculated ? (<div>{calculated.nested.value}</div>) : null}</section>;
}
class Worker {
  run(): { ok: boolean } {
    return { ok: true };
  }
}
const handler = (value: number) => {
  return Math.round(value);
};
`;
	const findings = scanJavaScriptFunctions("sample.tsx", source);
	assert.deepEqual(
		findings.map(({ display }) => display),
		[
			"Panel (function)",
			"calculated (arrow)",
			"render (arrow)",
			"run (method)",
			"handler (arrow)",
		],
	);
	assert.equal(
		findings.find(({ display }) => display === "Panel (function)")?.lines,
		10,
	);
});

test("Python scanner ignores docstrings and respects indentation", () => {
	const source = `"""def hidden():\n    pass"""
async def outer(value):
    def inner():
        return value

    return inner()

result = outer(1)
`;
	const findings = scanPythonFunctions("sample.py", source);
	assert.deepEqual(
		findings.map(({ display, lines }) => [display, lines]),
		[
			["outer (function)", 5],
			["inner (function)", 2],
		],
	);
});

test("shell scanner ignores comments and quoted braces", () => {
	const source = `# hidden() { }
run() {
  local text="}"
  printf '%s\\n' "$text"
}
`;
	assert.deepEqual(
		scanShellFunctions("sample.sh", source).map(({ display, lines }) => [
			display,
			lines,
		]),
		[["run (function)", 4]],
	);
});

function sampleScan(fileLines = 1_201, functionLines = 151) {
	return {
		exemptions: [],
		files: [{ id: "large.rs", lines: fileLines, path: "large.rs" }],
		functions: [
			{
				id: "large.rs::rust:function:large:hash:1",
				display: "large (function)",
				lines: functionLines,
				startLine: 1,
			},
		],
	};
}

test("ratchet rejects new and growing hard-limit violations", () => {
	const baseline = baselineFor(sampleScan());
	assert.deepEqual(evaluateRatcheting(sampleScan(), baseline).failures, []);
	assert.match(
		evaluateRatcheting(sampleScan(1_202), baseline).failures[0],
		/grew/u,
	);
	assert.match(
		evaluateRatcheting(sampleScan(1_201, 152), baseline).failures[0],
		/grew/u,
	);
	assert.match(
		evaluateRatcheting(sampleScan(), { ...baseline, functions: {} })
			.failures[0],
		/new function/u,
	);
});

test("ratchet ignores test hard limits while retaining their design goals", () => {
	const testScan = {
		exemptions: [],
		files: [
			{ id: "tests/large.spec.ts", lines: 1_500, path: "tests/large.spec.ts" },
		],
		functions: [
			{
				id: "tests/large.spec.ts::javascript:function:large:hash:1",
				display: "large (function)",
				lines: 200,
				startLine: 1,
			},
		],
	};
	const baseline = baselineFor(testScan);
	const result = evaluateRatcheting(testScan, baseline);
	assert.deepEqual(baseline.files, {});
	assert.deepEqual(baseline.functions, {});
	assert.deepEqual(result.failures, []);
	assert.deepEqual(result.goals, { files: 1, functions: 1 });
});

test("ratchet makes resolved allowances stale and serializes deterministically", () => {
	const baseline = baselineFor(sampleScan());
	const result = evaluateRatcheting(
		{ exemptions: [], files: [], functions: [] },
		baseline,
	);
	assert.equal(result.stale.length, 2);
	assert.match(result.failures[0], /stale file/u);
	assert.deepEqual(JSON.parse(serializeBaseline(baseline)), baseline);
});
