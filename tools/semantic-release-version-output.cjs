const { appendFileSync } = require("node:fs");
const { execFileSync } = require("node:child_process");

function deleteBootstrapTag(tag) {
  if (!tag) return;
  execFileSync("git", ["tag", "--delete", tag], { stdio: "ignore" });
  delete process.env.LIGHT_SEMANTIC_RELEASE_BOOTSTRAP_TAG;
}

async function verifyRelease(_pluginConfig, context) {
  const { nextRelease } = context;
  const expected = process.env.LIGHT_EXPECTED_RELEASE_VERSION;
  if (expected && expected !== nextRelease.version) {
    throw new Error(
      `Semantic-release selected ${nextRelease.version}, expected ${expected} from the gated build`,
    );
  }

  deleteBootstrapTag(process.env.LIGHT_SEMANTIC_RELEASE_BOOTSTRAP_TAG);

  const output = process.env.LIGHT_RELEASE_OUTPUT;
  if (output) {
    appendFileSync(output, `version=${nextRelease.version}\nrelease=true\n`);
  }
}

module.exports = { verifyRelease };
