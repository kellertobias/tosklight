const {
	inspectReleaseEpoch,
	commitsAfterBaseline,
} = require("./release-epoch.cjs");

async function analyzeCommits(pluginConfig, context) {
	const epoch = inspectReleaseEpoch(context.cwd);
	const { analyzeCommits: conventionalAnalysis } = await import(
		"@semantic-release/commit-analyzer"
	);
	const release = await conventionalAnalysis(pluginConfig, {
		...context,
		commits: commitsAfterBaseline(context.commits, epoch, context.cwd),
	});
	// With retained historical 0.x tags, the first release-worthy change crosses to exactly 1.0.0.
	// Afterwards the maintained analyzer resumes its ordinary fix/feat/breaking SemVer rules.
	return release && !epoch.latest ? "major" : release;
}

module.exports = { analyzeCommits };
