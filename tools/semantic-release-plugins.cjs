const commitAnalyzer = [
  "@semantic-release/commit-analyzer",
  {
    preset: "conventionalcommits",
    releaseRules: [
      { breaking: true, release: "major" },
      { type: "fix", release: "patch" },
      { type: "perf", release: "patch" },
      { type: "feat", release: "minor" },
      { type: "docs", release: false },
      { type: "test", release: false },
      { type: "style", release: false },
      { type: "refactor", release: false },
      { type: "build", release: false },
      { type: "ci", release: false },
      { type: "chore", release: false },
    ],
  },
];

const releaseNotes = [
  "@semantic-release/release-notes-generator",
  { preset: "conventionalcommits" },
];

module.exports = {
  previewPlugins: [
    commitAnalyzer,
    releaseNotes,
    "./tools/semantic-release-version-output.cjs",
  ],
  releasePlugins: [
    commitAnalyzer,
    releaseNotes,
    "./tools/semantic-release-version-output.cjs",
    [
      "@semantic-release/github",
      {
        assets: [".artifacts/release/*"],
      },
    ],
  ],
};
