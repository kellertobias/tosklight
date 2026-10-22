import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";

const require = createRequire(import.meta.url);
const {
	inspectReleaseEpoch,
	commitsAfterBaseline,
	verifyEpochVersion,
} = require("./release-epoch.cjs");
const { previewPlugins } = require("./semantic-release-plugins.cjs");
const rules = previewPlugins[0][1];

function fixture() {
	mkdirSync(artifactPaths.tmp, { recursive: true });
	const cwd = mkdtempSync(path.join(artifactPaths.tmp, "release-epoch-"));
	const git = (...args) =>
		execFileSync("git", args, {
			cwd,
			encoding: "utf8",
			stdio: ["ignore", "pipe", "pipe"],
		}).trim();
	git("init", "-b", "main");
	git("config", "user.email", "release-test@example.invalid");
	git("config", "user.name", "Release policy test");
	const commit = (message) => {
		git("commit", "--allow-empty", "-m", message);
		return { hash: git("rev-parse", "HEAD"), message };
	};
	const historical = commit("feat!: historical incompatible changes");
	git("tag", "v0.62.0");
	git("tag", "documentation-captures");
	const baseline = commit("docs: reviewed release reset baseline");
	writeFileSync(
		path.join(cwd, "release-policy.json"),
		JSON.stringify({
			schemaVersion: 1,
			baselineCommit: baseline.hash,
			firstVersion: "1.0.0",
		}),
	);
	return {
		cwd,
		git,
		commit,
		historical,
		baseline,
		dispose: () => rmSync(cwd, { recursive: true, force: true }),
	};
}

async function preview(repo) {
	const remote = path.join(repo.cwd, "remote.git");
	repo.git("-c", "init.defaultBranch=master", "init", "--bare", remote);
	repo.git("push", "--quiet", remote, "HEAD:refs/heads/main", "--tags");
	repo.git("--git-dir", remote, "symbolic-ref", "HEAD", "refs/heads/main");
	const script = `
    import semanticRelease from ${JSON.stringify(pathToFileURL(require.resolve("semantic-release")).href)};
    import { createRequire } from "node:module";
    const require = createRequire(import.meta.url);
    const { analyzeCommits } = require(${JSON.stringify(require.resolve("./semantic-release-epoch-analyzer.cjs"))});
    const { inspectReleaseEpoch, verifyEpochVersion } = require(${JSON.stringify(require.resolve("./release-epoch.cjs"))});
    const result = await semanticRelease({
      branches: ["main"], tagFormat: "v\${version}",
      repositoryUrl: ${JSON.stringify(pathToFileURL(remote).href)}, dryRun: true, noCi: true,
      plugins: [[{ analyzeCommits }, ${JSON.stringify(rules)}], { verifyRelease(_config, context) {
        verifyEpochVersion(context.nextRelease.version, inspectReleaseEpoch(context.cwd));
      } }],
    });
    process.stdout.write("EPOCH_RESULT=" + JSON.stringify(result === false ? null : result.nextRelease.version) + "\\n");
  `;
	const env = {
		...process.env,
		CI: "true",
		GITHUB_ACTIONS: "true",
		GITHUB_REF: "refs/heads/main",
		GITHUB_SHA: repo.git("rev-parse", "HEAD"),
		GITHUB_EVENT_NAME: "push",
	};
	delete env.NODE_TEST_CONTEXT;
	const result = spawnSync(
		process.execPath,
		["--input-type=module", "--eval", script],
		{
			cwd: repo.cwd,
			env,
			encoding: "utf8",
			timeout: 30_000,
		},
	);
	assert.equal(result.status, 0, result.stdout + result.stderr);
	const line = result.stdout
		.split("\n")
		.find((value) => value.startsWith("EPOCH_RESULT="));
	assert.ok(line, result.stdout + result.stderr);
	return JSON.parse(line.slice("EPOCH_RESULT=".length));
}

test("a documentation-only change cannot release historical breaking commits", async () => {
	const repo = fixture();
	try {
		const doc = repo.commit("docs: clearer installation text");
		const epoch = inspectReleaseEpoch(repo.cwd);
		assert.equal(epoch.latest, null);
		assert.deepEqual(
			commitsAfterBaseline(
				[repo.historical, repo.baseline, doc],
				epoch,
				repo.cwd,
			),
			[doc],
		);
		assert.equal(await preview(repo), null);
		assert.equal(
			repo.git("tag", "--list", "v1*"),
			"",
			"dry run creates no release tag",
		);
	} finally {
		repo.dispose();
	}
});

test("the maintained release engine selects exactly 1.0.0 for the first fix, feature or breaking change", async () => {
	for (const message of [
		"fix: correct camera entry",
		"feat: add a playback control",
		"feat!: revise the control protocol",
	]) {
		const repo = fixture();
		try {
			repo.commit(message);
			const before = repo.git("show-ref", "--tags");
			assert.equal(await preview(repo), "1.0.0", message);
			assert.equal(
				repo.git("show-ref", "--tags"),
				before,
				"preserve 0.x and documentation capture refs",
			);
		} finally {
			repo.dispose();
		}
	}
});

test("a published current-line 1.0.0 resumes ordinary patch, minor and major semantics", async () => {
	for (const [message, expected] of [
		["fix: correct a control", "1.0.1"],
		["feat: add a control", "1.1.0"],
		["feat!: change the protocol", "2.0.0"],
	]) {
		const repo = fixture();
		try {
			repo.commit("fix: first reviewed beta");
			repo.git("tag", "-a", "v1.0.0", "-m", "Reviewed beta");
			repo.commit(message);
			assert.equal(await preview(repo), expected);
		} finally {
			repo.dispose();
		}
	}
});

test("the production dry-run wrapper supports a master default branch without changing release refs", () => {
	const repo = fixture();
	try {
		repo.commit("fix: first beta with a different Git default branch");
		const before = repo.git("show-ref", "--tags");
		const remote = path.join(repo.cwd, "origin.git");
		repo.git("init", "--bare", "-b", "main", remote);
		repo.git("push", "--quiet", remote, "HEAD:refs/heads/main", "--tags");
		repo.git("remote", "add", "origin", remote);
		symlinkSync(
			path.dirname(require.resolve("./release-epoch.cjs")),
			path.join(repo.cwd, "tools"),
			"dir",
		);
		const output = path.join(repo.cwd, "release-output");
		const env = {
			...process.env,
			CI: "true",
			GITHUB_ACTIONS: "true",
			GITHUB_REF: "refs/heads/main",
			GITHUB_SHA: repo.git("rev-parse", "HEAD"),
			GITHUB_EVENT_NAME: "push",
			GIT_CONFIG_COUNT: "1",
			GIT_CONFIG_KEY_0: "init.defaultBranch",
			GIT_CONFIG_VALUE_0: "master",
			LIGHT_RELEASE_OUTPUT: output,
			LIGHT_EXPECTED_RELEASE_VERSION: "1.0.0",
		};
		delete env.NODE_TEST_CONTEXT;
		const result = spawnSync(
			process.execPath,
			[require.resolve("./run-semantic-release.mjs"), "--dry-run"],
			{
				cwd: repo.cwd,
				env,
				encoding: "utf8",
				timeout: 30_000,
			},
		);
		assert.equal(result.status, 0, result.stdout + result.stderr);
		assert.equal(readFileSync(output, "utf8"), "version=1.0.0\nrelease=true\n");
		assert.equal(
			repo.git("show-ref", "--tags"),
			before,
			"dry run preserves every release ref",
		);
		assert.equal(
			repo.git("--git-dir", remote, "symbolic-ref", "HEAD"),
			"refs/heads/main",
		);
	} finally {
		repo.dispose();
	}
});

test("stale 1.x and 2.x tags fail closed in a full-history checkout", () => {
	const repo = fixture();
	try {
		for (const tag of ["v1.0.0", "v2.6.3"])
			repo.git("tag", tag, repo.historical.hash);
		const before = repo.git("show-ref", "--tags");
		assert.throws(
			() => inspectReleaseEpoch(repo.cwd),
			/Historical application tags remain:.*v1.0.0.*v2.6.3/u,
		);
		assert.equal(
			repo.git("show-ref", "--tags"),
			before,
			"policy never deletes or rewrites refs",
		);
	} finally {
		repo.dispose();
	}
});

test("a higher post-baseline tag cannot stand in for the first 1.0.0 release", () => {
	const repo = fixture();
	try {
		repo.commit("fix: beta");
		repo.git("tag", "v2.6.3");
		assert.throws(
			() => inspectReleaseEpoch(repo.cwd),
			/must begin with v1.0.0/u,
		);
		assert.throws(
			() =>
				verifyEpochVersion("2.0.0", { firstVersion: "1.0.0", latest: null }),
			/First release must be 1.0.0/u,
		);
	} finally {
		repo.dispose();
	}
});

test("the baseline must belong to the checked out history", () => {
	const repo = fixture();
	try {
		writeFileSync(
			path.join(repo.cwd, "release-policy.json"),
			JSON.stringify({
				schemaVersion: 1,
				baselineCommit: "a".repeat(40),
				firstVersion: "1.0.0",
			}),
		);
		assert.throws(() => inspectReleaseEpoch(repo.cwd));
	} finally {
		repo.dispose();
	}
});

test("a resurrected old 2.x tag on the baseline itself cannot outrank the new 1.0.0", () => {
	const repo = fixture();
	try {
		repo.commit("fix: first new beta");
		repo.git("tag", "v1.0.0");
		repo.git("tag", "v2.6.3", repo.baseline.hash);
		assert.throws(
			() => inspectReleaseEpoch(repo.cwd),
			/Historical application tags remain: v2.6.3/u,
		);
	} finally {
		repo.dispose();
	}
});
