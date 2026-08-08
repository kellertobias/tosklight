import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

// The build output is what the Rust server embeds, so it goes to the canonical frontend
// artifact directory rather than a local `dist/`.
export default defineConfig({
	cacheDir: `${artifactPaths.viteCache}/media`,
	plugins: [react()],
	build: { outDir: artifactPaths.mediaFrontend, emptyOutDir: true },
	resolve: { dedupe: ["react", "react-dom"] },
	server: {
		port: 4178,
		strictPort: true,
		// During development the React application talks to a Media Server started separately,
		// so the API is proxied instead of duplicated.
		proxy: { "/api": { target: "http://127.0.0.1:8080" } },
	},
	preview: { port: 4178, strictPort: true },
	clearScreen: false,
});
