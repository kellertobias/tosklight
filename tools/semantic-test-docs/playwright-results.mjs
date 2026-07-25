import fs from "node:fs";

export function readPlaywrightResults(file) {
	if (!file) return new Map();
	const report = JSON.parse(fs.readFileSync(file, "utf8"));
	const results = new Map();
	for (const suite of report.suites ?? [])
		collectSuite(suite, results, suite.file);
	return results;
}

function collectSuite(suite, results, inheritedFile) {
	const sourceFile = suite.file ?? inheritedFile;
	for (const spec of suite.specs ?? []) {
		const identity = scenarioIdentity(spec.title);
		if (!identity) continue;
		const attempts = (spec.tests ?? []).flatMap((test) => test.results ?? []);
		const last = attempts.at(-1);
		const statuses = (spec.tests ?? [])
			.map((test) => test.status)
			.filter(Boolean);
		const status = statuses.includes("unexpected")
			? "failed"
			: statuses.includes("flaky")
				? "flaky"
				: statuses.includes("expected")
					? "passed"
					: statuses.length && statuses.every((value) => value === "skipped")
						? "skipped"
						: (last?.status ?? "unknown");
		const value = {
			status,
			durationMs: attempts.reduce(
				(total, attempt) => total + (Number(attempt.duration) || 0),
				0,
			),
			attempts: attempts.length,
		};
		const key = `${normalSource(sourceFile)}\0${identity.id}\0${identity.title}`;
		results.set(key, value);
	}
	for (const child of suite.suites ?? [])
		collectSuite(child, results, sourceFile);
}

function scenarioIdentity(title) {
	const match = /^([A-Z][A-Z0-9-]+)\s+@bench\s+@ui\s+›\s+(.+)$/u.exec(title);
	return match ? { id: match[1], title: match[2] } : undefined;
}

function normalSource(file = "") {
	return file.replaceAll("\\", "/").split("/").at(-1);
}

export function resultFor(results, source, id, title) {
	return results.get(`${normalSource(source)}\0${id}\0${title}`) ?? null;
}
