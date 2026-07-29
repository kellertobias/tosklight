import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const RUNTIME_ROOT = "crates/light/adapters/headless/src/runtime";
const APPLICATION_ROOT = "crates/light/src";

const RAW_OWNER_TYPE =
	/\b(?:Mutex|RwLock|MutexGuard|RwLockReadGuard|RwLockWriteGuard|DeskStore|ShowStore|FixtureLibrary|ProgrammerRegistry|HighlightRegistry)\b/u;
const LOCK_BEARING_RETURN =
	/\b(?:Mutex|RwLock|MutexGuard|RwLockReadGuard|RwLockWriteGuard|DeskStore|ShowStore|FixtureLibrary|ProgrammerRegistry|HighlightRegistry|ActiveShowService|ShowPatchService|SelectiveShowImportService)\b/u;
const CONCRETE_RESOURCE_FIELD =
	/\bpub(?:\([^)]*\))?\s+(?:service|patch|selective_import|runtime_service|speed_group_service|engine|rate|network|manual_clock|test_clock_lock)\s*:\s*[^,\n]*(?:ActiveShowService|ShowPatchService|SelectiveShowImportService|OutputRuntimeService|SpeedGroupService|Engine|AtomicU16|NetworkOutput|ManualClock|Mutex\s*<\s*\(\s*\)\s*>)[^,\n]*/gu;
const CONCRETE_RESOURCE_RETURN =
	/\bpub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*[\s\S]{0,500}?->\s*[^;{\n]*(?:ActiveShowService|ShowPatchService|SelectiveShowImportService|OutputRuntimeService|SpeedGroupService|Engine|AtomicU16|NetworkOutput|ManualClock)\b[^;{\n]*/gu;
const PUBLIC_RESOURCE_SIGNATURE =
	/\bpub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)[\s\S]{0,1000}?(?=\{)/gu;
const CONCRETE_ACTIVE_SHOW_SERVICE =
	/\b(?:ActiveShowService|ShowPatchService|SelectiveShowImportService)\b/u;

function debtInventory(source) {
	return Object.fromEntries(
		source
			.trim()
			.split(/\r?\n/u)
			.filter(Boolean)
			.map((line) => {
				const match = line.match(/^(.*)=(\d+)$/u);
				if (!match) throw new Error(`invalid capability-state debt entry: ${line}`);
				return [match[1], Number(match[2])];
			}),
	);
}

// Temporary Plan 13 debt. Entries are exact per-file/category occurrence ceilings: additions
// fail, while removals make the inventory stale so it must shrink in the same change.
export const CAPABILITY_STATE_DEBT = {
	appStateFields: {},
	adapterAccess: {},
	publicApis: {},
	resourceEscapes: {},
	taskOwnership: {},
};

function normalizePath(value) {
	return value.split(path.sep).join("/");
}

function isTestSource(file) {
	return (
		file.includes("/tests/") ||
		file.endsWith("/tests.rs") ||
		file.endsWith("_tests.rs")
	);
}

function withoutInlineTests(source) {
	const inlineTests = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+tests\s*\{/u.exec(
		source,
	);
	return inlineTests ? source.slice(0, inlineTests.index) : source;
}

function isCapabilityResource(file) {
	return (
		file === `${RUNTIME_ROOT}/capability_resources.rs` ||
		new RegExp(
			`^${RUNTIME_ROOT}/capabilities/[a-z0-9_]+/(?:resource|repository)\\.rs$`,
			"u",
		).test(file)
	);
}

function isCompositionRoot(file) {
	return (
		file === `${RUNTIME_ROOT}/bootstrap.rs` ||
		file === `${RUNTIME_ROOT}/startup_state.rs`
	);
}

function isCapabilitySupervisor(file) {
	return new RegExp(
		`^${RUNTIME_ROOT}/capabilities/[a-z0-9_]+/supervisor\\.rs$`,
		"u",
	).test(file);
}

function isBoundedLocalTaskOwner(file) {
	// These are deliberately not application-lifecycle tasks:
	// - the released benchmark owns and joins its receiver thread within one benchmark run;
	// - the test-bench scheduler runs inline inside one bounded HTTP request and returns only
	//   after its local cancellation token has stopped the scheduler;
	// - each visualization transport owns and joins its capacity-one socket writer.
	return (
		file === "apps/light-headless/src/bin/light_benchmark/loopback.rs" ||
		file === `${RUNTIME_ROOT}/test_bench.rs` ||
		file === `${RUNTIME_ROOT}/visualization_transport.rs`
	);
}

function addCount(counts, key, amount = 1) {
	counts.set(key, (counts.get(key) ?? 0) + amount);
}

function topLevelStructBody(source, structName) {
	const declaration = new RegExp(`\\bstruct\\s+${structName}\\s*\\{`, "u").exec(
		source,
	);
	if (!declaration) return "";
	const open = source.indexOf("{", declaration.index);
	let depth = 0;
	for (let index = open; index < source.length; index += 1) {
		if (source[index] === "{") depth += 1;
		if (source[index] === "}") {
			depth -= 1;
			if (depth === 0) return source.slice(open + 1, index);
		}
	}
	return source.slice(open + 1);
}

export function appStateRawFields(source) {
	const body = topLevelStructBody(source, "AppState");
	const fields = new Map();
	let declaration = "";
	for (const rawLine of body.split(/\r?\n/u)) {
		const line = rawLine.trim();
		if (
			!line ||
			line.startsWith("///") ||
			line.startsWith("#[") ||
			(!declaration && !line.startsWith("pub(super)"))
		)
			continue;
		declaration += `${declaration ? " " : ""}${line}`;
		if (!line.endsWith(",")) continue;
		const match = declaration.match(
			/^pub\(super\)\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(.+),$/u,
		);
		if (match && RAW_OWNER_TYPE.test(match[2])) {
			fields.set(`${match[1]}: ${match[2].replace(/\s+/gu, " ").trim()}`, 1);
		}
		declaration = "";
	}
	return fields;
}

function adapterRawAccess(entries) {
	const counts = new Map();
	const stateLock =
		/\b(?:state|self\.state)\s*\.\s*([A-Za-z_][A-Za-z0-9_]*)(?:(?!;|\n\n)[\s\S]){0,180}?\.\s*(?:lock|lock_owned|try_lock|try_lock_owned|read|write|try_read|try_write)\s*\(/gu;
	const compatibilityLock =
		/\b(?:state|self\.state)\s*\.\s*programming\s*\.\s*desk_lock\s*\(/gu;
	const rawStore =
		/\b(?:DeskStore|ShowStore|FixtureLibrary|ProgrammerRegistry|HighlightRegistry)(?:::\s*(?:open|new|default))?\b/gu;

	for (const entry of entries) {
		const file = normalizePath(entry.path);
		if (isCapabilityResource(file)) {
			for (const match of entry.source.matchAll(CONCRETE_RESOURCE_FIELD))
				addCount(
					counts,
					`${file}|concrete-resource-field:${match[0].replace(/\s+/gu, " ")}`,
				);
			for (const match of entry.source.matchAll(CONCRETE_RESOURCE_RETURN))
				addCount(
					counts,
					`${file}|concrete-resource-return:${match[0].replace(/\s+/gu, " ")}`,
				);
			for (const match of entry.source.matchAll(PUBLIC_RESOURCE_SIGNATURE)) {
				if (
					match[1] !== "new" &&
					CONCRETE_ACTIVE_SHOW_SERVICE.test(match[0])
				) {
					addCount(
						counts,
						`${file}|concrete-active-show-signature:${match[0].replace(/\s+/gu, " ")}`,
					);
				}
			}
			continue;
		}
		if (
			isTestSource(file) ||
			!file.startsWith(`${RUNTIME_ROOT}/`)
		)
			continue;
		for (const _match of entry.source.matchAll(stateLock))
			addCount(counts, `${file}|state-lock`);
		for (const _match of entry.source.matchAll(compatibilityLock))
			addCount(counts, `${file}|compatibility-desk-lock`);
		if (!isCompositionRoot(file)) {
			for (const match of entry.source.matchAll(rawStore))
				addCount(counts, `${file}|raw-owner:${match[0].replace(/\s+/gu, "")}`);
		}
	}
	return counts;
}

function publicLockBearingApis(entries) {
	const counts = new Map();
	const publicFunction =
		/\bpub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)[\s\S]{0,500}?->\s*([^;{]+)(?:\{|where\b)/gu;
	for (const entry of entries) {
		const file = normalizePath(entry.path);
		if (isTestSource(file) || isCapabilityResource(file)) continue;
		for (const match of entry.source.matchAll(publicFunction)) {
			const returnType = match[2].replace(/\s+/gu, " ").trim();
			if (LOCK_BEARING_RETURN.test(returnType))
				addCount(counts, `${file}|${match[1]} -> ${returnType}`);
		}
	}
	return counts;
}

function capabilityResourceEscapes(entries) {
	const counts = new Map();
	const resourceDeclaration =
		/\bstruct\s+([A-Za-z_][A-Za-z0-9_]*Resource)\s*\{/gu;
	const derefImplementation =
		/\bimpl(?:\s*<[^>{}]*>)?\s+(?:std::ops::)?Deref\s+for\s+([A-Za-z_][A-Za-z0-9_]*Resource)\b[\s\S]{0,500}?\btype\s+Target\s*=\s*([^;]+);/gu;

	for (const entry of entries) {
		const file = normalizePath(entry.path);
		if (isTestSource(file) || !file.startsWith(`${RUNTIME_ROOT}/`)) continue;
		for (const match of entry.source.matchAll(resourceDeclaration)) {
			const resource = match[1];
			const body = topLevelStructBody(entry.source.slice(match.index), resource);
			let declaration = "";
			for (const rawLine of body.split(/\r?\n/u)) {
				const line = rawLine.trim();
				if (!line || line.startsWith("///") || line.startsWith("#[")) continue;
				if (!declaration && !line.startsWith("pub")) continue;
				declaration += `${declaration ? " " : ""}${line}`;
				if (!line.endsWith(",")) continue;
				const field = declaration.match(
					/^pub(?:\([^)]*\))?\s+([A-Za-z_][A-Za-z0-9_]*)\s*:/u,
				);
				if (field)
					addCount(counts, `${file}|public-field:${resource}.${field[1]}`);
				declaration = "";
			}
		}
		for (const match of entry.source.matchAll(derefImplementation)) {
			addCount(
				counts,
				`${file}|deref:${match[1]}->${match[2].replace(/\s+/gu, " ").trim()}`,
			);
		}
	}
	return counts;
}

function backgroundTaskOwnership(entries) {
	const counts = new Map();
	const patterns = [
		["tokio-spawn", /\btokio::spawn\s*\(/gu],
		["join-handle", /\bJoinHandle\s*</gu],
		["cancellation-root", /\bCancellationToken::new\s*\(/gu],
	];
	for (const entry of entries) {
		const file = normalizePath(entry.path);
		if (
			isTestSource(file) ||
			(!file.startsWith(`${RUNTIME_ROOT}/`) &&
				!file.startsWith("apps/light-headless/src/")) ||
			isCapabilityResource(file) ||
			isCapabilitySupervisor(file) ||
			isBoundedLocalTaskOwner(file) ||
			file === `${RUNTIME_ROOT}/bootstrap.rs`
		)
			continue;
		const source = withoutInlineTests(entry.source);
		for (const [label, expression] of patterns) {
			const matches = source.match(expression);
			if (matches?.length) addCount(counts, `${file}|${label}`, matches.length);
		}
	}
	return counts;
}

function compareInventory(rule, observed, expected) {
	const failures = [];
	const expectedMap = new Map(Object.entries(expected));
	for (const [key, count] of observed) {
		const ceiling = expectedMap.get(key);
		if (ceiling === undefined)
			failures.push(`${rule}: new ownership debt ${key} (${count})`);
		else if (count > ceiling)
			failures.push(
				`${rule}: ownership debt grew ${key} (${ceiling} -> ${count})`,
			);
		else if (count < ceiling)
			failures.push(
				`${rule}: stale ownership debt ${key} (${ceiling} -> ${count}); shrink the inventory`,
			);
		expectedMap.delete(key);
	}
	for (const [key, count] of expectedMap)
		failures.push(
			`${rule}: stale ownership debt ${key} (${count} -> 0); shrink the inventory`,
		);
	return failures;
}

export function capabilityStateBoundaryFailures(
	entries,
	{
		appStateSource = entries.find(
			(entry) =>
				normalizePath(entry.path) === `${RUNTIME_ROOT}/state.rs`,
		)?.source ?? "",
		debt = CAPABILITY_STATE_DEBT,
	} = {},
) {
	const observed = observedCapabilityStateDebt(entries, { appStateSource });
	return [
		...compareInventory(
			"AppState raw field",
			new Map(Object.entries(observed.appStateFields)),
			debt.appStateFields,
		),
		...compareInventory(
			"adapter raw access",
			new Map(Object.entries(observed.adapterAccess)),
			debt.adapterAccess,
		),
		...compareInventory(
			"lock-bearing public API",
			new Map(Object.entries(observed.publicApis)),
			debt.publicApis,
		),
		...compareInventory(
			"capability resource escape",
			new Map(Object.entries(observed.resourceEscapes)),
			debt.resourceEscapes,
		),
		...compareInventory(
			"background task ownership",
			new Map(Object.entries(observed.taskOwnership)),
			debt.taskOwnership,
		),
	].sort();
}

function countsObject(counts) {
	return Object.fromEntries([...counts].sort(([left], [right]) => left.localeCompare(right)));
}

export function observedCapabilityStateDebt(
	entries,
	{
		appStateSource = entries.find(
			(entry) =>
				normalizePath(entry.path) === `${RUNTIME_ROOT}/state.rs`,
		)?.source ?? "",
	} = {},
) {
	return {
		appStateFields: countsObject(appStateRawFields(appStateSource)),
		adapterAccess: countsObject(adapterRawAccess(entries)),
		publicApis: countsObject(publicLockBearingApis(entries)),
		resourceEscapes: countsObject(capabilityResourceEscapes(entries)),
		taskOwnership: countsObject(backgroundTaskOwnership(entries)),
	};
}

function walk(directory) {
	if (!fs.existsSync(directory)) return [];
	return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const target = path.join(directory, entry.name);
		return entry.isDirectory() ? walk(target) : [target];
	});
}

export function readCapabilityStateBoundarySources(repositoryRoot) {
	const roots = [
		path.join(repositoryRoot, RUNTIME_ROOT),
		path.join(repositoryRoot, APPLICATION_ROOT),
		path.join(repositoryRoot, "apps/light-headless/src"),
	];
	return roots
		.flatMap(walk)
		.filter((file) => file.endsWith(".rs"))
		.map((file) => ({
			path: normalizePath(path.relative(repositoryRoot, file)),
			source: fs.readFileSync(file, "utf8"),
		}));
}

function cli() {
	const repositoryRoot = path.resolve(
		path.dirname(fileURLToPath(import.meta.url)),
		"..",
	);
	const entries = readCapabilityStateBoundarySources(repositoryRoot);
	if (process.argv.includes("--print-debt")) {
		console.log(JSON.stringify(observedCapabilityStateDebt(entries), null, 2));
		return;
	}
	const failures = capabilityStateBoundaryFailures(entries);
	for (const failure of failures) console.error(`architecture error: ${failure}`);
	if (failures.length) process.exitCode = 1;
	else console.log("Capability state ownership boundaries are valid.");
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
	cli();
