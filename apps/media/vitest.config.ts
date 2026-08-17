import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

const isCi = Boolean(
	(globalThis as { process?: { env?: Record<string, string | undefined> } })
		.process?.env?.CI,
);

export default defineConfig({
	cacheDir: `${artifactPaths.viteCache}/media-vitest`,
	plugins: [react()],
	resolve: { dedupe: ["react", "react-dom"] },
	test: {
		environment: "jsdom",
		setupFiles: "./src/testing/setup.ts",
		css: true,
		testTimeout: isCi ? 20_000 : 5_000,
	},
});
