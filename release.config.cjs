const { releasePlugins } = require("./tools/semantic-release-plugins.cjs");

module.exports = {
  branches: ["main"],
  repositoryUrl: "https://github.com/kellertobias/tosklight.git",
  tagFormat: "v${version}",
  plugins: releasePlugins,
};
