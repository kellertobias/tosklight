export const LIMITS = Object.freeze({
	file: 1_200,
	fileGoal: 400,
	function: 150,
	functionGoal: 20,
});

const SOURCE_ROOT = /^(?:apps|crates|packages)\//u;
// `generated` joins these because a generator's output is not something anyone can choose to
// write more briefly: the file is as long as the contract it mirrors, and the only way to shrink
// it would be to say less in the contract.
const EXCLUDED_DIRECTORY =
	/(?:^|\/)(?:artifacts|assets|docs|experiments|generated)(?:\/|$)/u;
const SOURCE_EXTENSION = /\.(?:js|py|rs|ts|tsx)$/iu;
const TEST_DIRECTORY =
	/(^|\/)(?:__tests__|e2e|stories|testing|[a-z0-9_]*tests)(?:\/|$)/u;
const TEST_FILENAME =
	/(?:^|\.)(?:spec|test|stories)\.[^.]+$|(?:^|_)(?:test|tests)\.rs$/u;

export function isSourcePath(repositoryPath) {
	return (
		SOURCE_ROOT.test(repositoryPath) &&
		!EXCLUDED_DIRECTORY.test(repositoryPath) &&
		SOURCE_EXTENSION.test(repositoryPath)
	);
}

export function isTestSource(repositoryPath) {
	const basename = repositoryPath.split("/").at(-1) ?? "";
	return TEST_DIRECTORY.test(repositoryPath) || TEST_FILENAME.test(basename);
}

export function functionLanguage(repositoryPath) {
	const extension = repositoryPath.split(".").at(-1)?.toLowerCase();
	if (extension === "rs") return "rust";
	if (["js", "ts", "tsx"].includes(extension)) return "javascript";
	if (extension === "py") return "python";
	return undefined;
}
