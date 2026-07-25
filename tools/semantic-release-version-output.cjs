const { appendFileSync } = require("node:fs");
const { execFileSync } = require("node:child_process");

function deleteBootstrapTag(tag) {
  if (!tag) return;
  execFileSync("git", ["tag", "--delete", tag], { stdio: "ignore" });
  delete process.env.LIGHT_SEMANTIC_RELEASE_BOOTSTRAP_TAG;
}

function verifyNewestMainCommit() {
  if (process.env.GITHUB_ACTIONS !== "true") return;
  const expected = process.env.GITHUB_SHA;
  if (!expected) throw new Error("GITHUB_SHA is required in GitHub Actions");

  const output = execFileSync("git", ["ls-remote", "origin", "refs/heads/main"], {
    encoding: "utf8",
  }).trim();
  const actual = output.split(/\s+/u)[0];
  if (actual !== expected) {
    throw new Error(
      `Refusing to release superseded commit ${expected}; GitHub main is now ${actual || "missing"}`,
    );
  }
}

async function verifyRelease(_pluginConfig, context) {
  const { nextRelease } = context;
  const expected = process.env.LIGHT_EXPECTED_RELEASE_VERSION;
  if (expected && expected !== nextRelease.version) {
    throw new Error(
      `Semantic-release selected ${nextRelease.version}, expected ${expected} from the gated build`,
    );
  }

  verifyNewestMainCommit();
  deleteBootstrapTag(process.env.LIGHT_SEMANTIC_RELEASE_BOOTSTRAP_TAG);

  const output = process.env.LIGHT_RELEASE_OUTPUT;
  if (output) {
    appendFileSync(output, `version=${nextRelease.version}\nrelease=true\n`);
  }
}

module.exports = { verifyRelease };
