#!/usr/bin/env node

import path from "node:path";
import { build } from "esbuild";
import { artifactRoot, repositoryRoot } from "./artifact-paths.mjs";

const output = path.join(
	artifactRoot,
	"build",
	"patch-mcp",
	"tosklight-patch-mcp.mjs",
);

await build({
	entryPoints: [path.join(repositoryRoot, "apps", "patch-mcp", "src", "main.ts")],
	bundle: true,
	platform: "node",
	format: "esm",
	outfile: output,
});

console.log(`Architect MCP bridge built: ${output}`);
