import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { artifactPaths } from "./artifact-paths.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = await readFile(
	process.env.LIGHT_DESKTOP_PERFORMANCE_TEST_SOURCE ??
		path.join(root, "tools/run-desktop-performance.mjs"),
	"utf8",
);
const temporaryRoot = artifactPaths.tmp;
await mkdir(temporaryRoot, { recursive: true });
const scratch = await mkdtemp(
	path.join(temporaryRoot, "desktop-performance-test-"),
);
after(() => rm(scratch, { recursive: true, force: true }));
function load(name, end, extra = {}) {
	const start = source.indexOf(`async function ${name}(`);
	assert.notEqual(start, -1, `production ${name} must exist`);
	const code = source.slice(start, source.indexOf(end, start));
	const context = {
		mkdir,
		mkdtemp,
		readFile,
		writeFile,
		path,
		options: { "output-dir": scratch },
		Date,
		JSON,
		Set,
		setTimeout,
		spawnSync: () => ({ status: null, error: { code: "ENOENT" } }),
		assertRunning: () => {},
		...extra,
	};
	return vm.runInNewContext(`${code}\n${name}`, context);
}
test("production failure evidence survives normal scratch cleanup and remains explicitly incomplete", async () => {
	const retain = load("retainCaseFailure", "\nfunction applicationInvocation(");
	const temporary = await mkdtemp(path.join(scratch, "case-"));
	const reportPath = path.join(temporary, "sheet.jsonl");
	const preparedPath = path.join(temporary, "prepared");
	const raw = '{"kind":"stage-started","recordedAt":"2026-10-14T10:51:00Z"}\n';
	await writeFile(reportPath, raw);
	await writeFile(preparedPath, '{"fixtureRecords":264}\n');
	const filename = await retain({
		performanceCase: { caseId: "demo" },
		executionMode: "unrestricted",
		state: {
			phase: "fixture-sheet-readiness",
			backendReady: true,
			prepared: true,
			sheet: { ready: false },
		},
		reportPath,
		preparedPath,
		child: { pid: 999, exitCode: null, signalCode: null },
		output: ["actual log\n"],
		error: "Fixture Sheet never reported itself ready",
	});
	await rm(temporary, { recursive: true });
	const saved = JSON.parse(await readFile(filename, "utf8"));
	assert.equal(saved.complete, false);
	assert.equal(saved.state.sheet.ready, false);
	assert.equal(await readFile(saved.files["fixture-sheet.jsonl"], "utf8"), raw);
	assert.equal(await readFile(saved.files.processLog, "utf8"), "actual log\n");
	assert.equal(saved.files.screenshot.unavailable, "ENOENT");
	assert.ok(!filename.startsWith(`${temporary}/`));
});
test("missing startup files retain honest unavailable state and original error", async () => {
	const retain = load("retainCaseFailure", "\nfunction applicationInvocation(");
	const filename = await retain({
		performanceCase: { caseId: "demo" },
		executionMode: "unrestricted",
		state: { phase: "backend-readiness" },
		reportPath: path.join(scratch, "absent"),
		preparedPath: path.join(scratch, "absent-marker"),
		child: { pid: 9, exitCode: 1, signalCode: null },
		output: [],
		error: "released Desk exited early (1)",
	});
	const saved = JSON.parse(await readFile(filename, "utf8"));
	assert.equal(saved.error, "released Desk exited early (1)");
	assert.equal(saved.files["fixture-sheet.jsonl"].unavailable, "ENOENT");
	assert.equal(saved.files["prepared.json"].unavailable, "ENOENT");
});
test("actual ready evidence satisfies unchanged exact count gate and is observed", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [264] },
	);
	const p = path.join(scratch, "ready.jsonl");
	await writeFile(
		p,
		'{"kind":"fixture-sheet-ready","fixtureRecords":264,"rowCount":264}\n',
	);
	let observed;
	await wait(p, 264, {}, 100, (s) => {
		observed = s;
	});
	assert.equal(observed.ready, true);
	assert.equal(observed.lastRecords[0].fixtureRecords, 264);
});
test("wrong count fails unchanged gate while preserving exact observed count", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [263] },
	);
	const p = path.join(scratch, "wrong.jsonl");
	await writeFile(p, '{"kind":"fixture-sheet-ready","fixtureRecords":263}\n');
	let observed;
	await assert.rejects(
		wait(p, 264, {}, 100, (s) => {
			observed = s;
		}),
		/did not converge to 264/,
	);
	assert.equal(observed.ready, false);
	assert.equal(observed.lastRecords[0].fixtureRecords, 263);
});
test("missing report retains uncertainty instead of invented loading predicate", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [] },
	);
	let observed;
	await assert.rejects(
		wait(path.join(scratch, "not-created"), null, {}, 100, (s) => {
			observed = s;
		}),
		/never reported itself ready/,
	);
	assert.equal(observed.reportPresent, false);
	assert.equal(observed.ready, false);
	assert.ok(observed.unobservedPredicates.includes("group-runtime loading"));
});
test("production runner writes prior successful cases as incomplete on later failure without successful aggregate", async () => {
	const out = await mkdtemp(path.join(scratch, "aggregate-"));
	const start = source.indexOf("const results = [];");
	const end = source.indexOf("\nfunction parseArguments(", start);
	const code = source.slice(start, end);
	const context = {
		writeFile,
		path,
		options: { "output-dir": out },
		JSON,
		Error,
		String,
		CASES: [
			{ caseId: "first", demo: false },
			{ caseId: "second", demo: false },
		],
		runCase: async (c) => {
			if (c.caseId === "second") throw new Error("observed missing ready");
			return { case_id: "first", achieved_rate_hz: 40 };
		},
	};
	await assert.rejects(
		vm.runInNewContext(`(async()=>{${code}})()`, context),
		/observed missing ready/,
	);
	const partial = JSON.parse(
		await readFile(path.join(out, "desktop-scenarios.partial.json"), "utf8"),
	);
	assert.equal(partial.complete, false);
	assert.equal(partial.scenarios.length, 1);
	assert.equal(partial.failed_case.case_id, "second");
	await assert.rejects(
		readFile(path.join(out, "desktop-scenarios.json")),
		/ENOENT/,
	);
});
test("partial report IO failure preserves the original benchmark failure", async () => {
	const start = source.indexOf("const results = [];");
	const end = source.indexOf("\nfunction parseArguments(", start);
	const code = source.slice(start, end);
	const original = new Error(
		"precise fixture-sheet failure; diagnostics path retained",
	);
	const diagnostic = new Error("disk full writing partial aggregate");
	diagnostic.code = "ENOSPC";
	const messages = [];
	const context = {
		writeFile: async () => {
			throw diagnostic;
		},
		path,
		options: { "output-dir": scratch },
		JSON,
		Error,
		String,
		console: { error: (message) => messages.push(message) },
		CASES: [{ caseId: "demo", demo: false }],
		runCase: async () => {
			throw original;
		},
	};
	await assert.rejects(
		vm.runInNewContext(`(async()=>{${code}})()`, context),
		(error) => error === original,
	);
	assert.equal(messages.length, 1);
	assert.match(messages[0], /partial.*unavailable/i);
	assert.match(messages[0], /disk full/);
});
