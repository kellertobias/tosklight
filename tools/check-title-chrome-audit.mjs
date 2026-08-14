import { readFileSync, readdirSync } from "node:fs";
import { extname, join, relative } from "node:path";

const root = process.cwd();
const sourceRoots = ["apps/light-desktop/src", "apps/ui-library/src"];
const intentionallyTitleless = new Map([
	[
		"apps/light-desktop/src/components/modals/DeskLockOverlay.tsx",
		"full-screen lock surface; its heading is page content and the close action is intentionally forbidden",
	],
]);
const primitiveOwners = new Set([
	"apps/ui-library/src/common/SearchBar.tsx",
	"apps/ui-library/src/modals/ModalStack.tsx",
	"apps/ui-library/src/window-kit/WindowKit.tsx",
]);

function filesBelow(directory) {
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = join(directory, entry.name);
		return entry.isDirectory() ? filesBelow(path) : [path];
	});
}

const files = sourceRoots
	.flatMap((directory) => filesBelow(join(root, directory)))
	.filter((file) => extname(file) === ".tsx")
	.filter((file) => !/\.(?:stories|test)\.tsx$/u.test(file));
const failures = [];
let windows = 0;
let modalTitles = 0;

for (const file of files) {
	const path = relative(root, file);
	const source = readFileSync(file, "utf8");
	windows += (source.match(/<(?:WindowHeader|WindowFrame|PaneView|PoolWindow)\b/gu) ?? []).length;
	modalTitles += (source.match(/<(?:ModalTitleBar|ModalFrame)\b/gu) ?? []).length;

	for (const match of source.matchAll(/<(?:WindowHeader|WindowFrame|PaneView|PoolWindow)\b[\s\S]*?>/gu)) {
		if (/\bactions\s*=/u.test(match[0]))
			failures.push(`${path}: legacy actions prop remains on window chrome`);
		if (/\bonSearch\s*=/u.test(match[0]))
			failures.push(`${path}: split onSearch prop remains on window chrome`);
	}
	for (const match of source.matchAll(/<(?:ModalTitleBar|ModalFrame)\b[\s\S]*?>/gu)) {
		if (/\b(?:actions|tabs|onSearch)\s*=/u.test(match[0]))
			failures.push(`${path}: legacy modal title prop remains`);
	}

	if (intentionallyTitleless.has(path) || primitiveOwners.has(path)) continue;
	for (const match of source.matchAll(
		/<(?:section|div)\b(?=[^>]*(?:role\s*=\s*["{]dialog|aria-modal|className\s*=\s*["{][^>]*(?:nested-modal|modal-card|file-confirmation)))[^>]*>/gu,
	)) {
		const nearby = source.slice(match.index, match.index + 2_000);
		if (!/<(?:ModalTitleBar|ModalFrame|ModalLayer|QuickSetupTitleBar|PlaybackLayoutControls)\b/u.test(nearby)) {
			const line = source.slice(0, match.index).split("\n").length;
			failures.push(`${path}:${line}: modal/dialog title is not owned by shared title chrome`);
		}
	}
}

if (windows < 30 || modalTitles < 80)
	failures.push(`inventory unexpectedly shrank: ${windows} window surfaces, ${modalTitles} modal title surfaces`);

if (failures.length) {
	console.error(failures.join("\n"));
	process.exitCode = 1;
} else {
	console.log(
		`Title chrome audit passed: ${windows} window surfaces, ${modalTitles} modal title surfaces, ${intentionallyTitleless.size} intentionally titleless surface.`,
	);
}
