const { execFileSync } = require("node:child_process");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const semver = require("semver");

function git(cwd, args) {
	return execFileSync("git", args, {
		cwd,
		encoding: "utf8",
		stdio: ["ignore", "pipe", "pipe"],
	}).trim();
}

function ancestor(cwd, older, newer) {
	try {
		git(cwd, ["merge-base", "--is-ancestor", older, newer]);
		return true;
	} catch (error) {
		if (error.status === 1) return false;
		throw error;
	}
}

/** A new release line starts after the reviewed baseline; historical 0.x tags stay intact. */
function inspectReleaseEpoch(cwd = process.cwd()) {
	const policy = JSON.parse(
		readFileSync(join(cwd, "release-policy.json"), "utf8"),
	);
	if (
		policy.schemaVersion !== 1 ||
		!/^[0-9a-f]{40}$/u.test(policy.baselineCommit) ||
		policy.firstVersion !== "1.0.0"
	) {
		throw new Error(
			"Invalid release-policy.json: expected a full baseline commit and first version 1.0.0",
		);
	}
	if (!ancestor(cwd, policy.baselineCommit, "HEAD")) {
		throw new Error(
			"Release baseline is not an ancestor of HEAD; fetch complete history before releasing",
		);
	}
	const current = [];
	const obsolete = [];
	for (const tag of git(cwd, ["tag", "--merged", "HEAD"]).split("\n")) {
		const version = tag.startsWith("v") ? semver.valid(tag.slice(1)) : null;
		if (!version || semver.lt(version, policy.firstVersion)) continue;
		const head = git(cwd, ["rev-list", "-n", "1", tag]);
		if (
			head !== policy.baselineCommit &&
			ancestor(cwd, policy.baselineCommit, head)
		)
			current.push({ tag, version, head });
		else obsolete.push(tag);
	}
	if (obsolete.length) {
		throw new Error(
			`Historical application tags remain: ${obsolete.join(", ")}. Reconcile the release reset on Forgejo and GitHub before releasing; refusing to select an old release line.`,
		);
	}
	if (
		current.length &&
		!current.some(({ version }) => version === policy.firstVersion)
	) {
		throw new Error(
			"The current release line must begin with v1.0.0; refusing a higher tag without that baseline",
		);
	}
	current.sort((a, b) => semver.rcompare(a.version, b.version));
	return { ...policy, latest: current[0] ?? null };
}

/** Old feature/breaking commits must not trigger the first release on a documentation-only push. */
function commitsAfterBaseline(commits, epoch, cwd = process.cwd()) {
	return commits.filter(
		({ hash }) => !ancestor(cwd, hash, epoch.baselineCommit),
	);
}

function verifyEpochVersion(version, epoch) {
	if (!epoch.latest && version !== epoch.firstVersion) {
		throw new Error(
			`First release must be ${epoch.firstVersion}, not ${version}`,
		);
	}
	if (epoch.latest && !semver.gt(version, epoch.latest.version)) {
		throw new Error(
			`Release ${version} must follow current-line ${epoch.latest.version}`,
		);
	}
}

module.exports = {
	inspectReleaseEpoch,
	commitsAfterBaseline,
	verifyEpochVersion,
};

if (require.main === module) {
	if (process.argv[2] !== "build-version")
		throw new Error("Usage: node tools/release-epoch.cjs build-version");
	const epoch = inspectReleaseEpoch();
	process.stdout.write(`${epoch.latest?.version ?? epoch.firstVersion}\n`);
}
