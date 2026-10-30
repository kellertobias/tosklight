import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const directory = path.join(artifactPaths.runtime, "demo-media");
const configuration = path.join(directory, "media-server.json");
await fs.mkdir(directory, { recursive: true });
try {
	await fs.copyFile(
		path.join(repositoryRoot, "assets/demo-media/media-server.json"),
		configuration,
		1,
	);
} catch (error) {
	if (error.code !== "EEXIST") throw error;
}
let binary = path.join(
	artifactPaths.cargo,
	"release",
	process.platform === "win32" ? "media-server.exe" : "media-server",
);
if (process.platform === "darwin") {
	const bundled = path.join(
		artifactPaths.cargo,
		"release/bundle/macos/ToskLight Pixel.app/Contents/MacOS/ToskLight Pixel",
	);
	try {
		await fs.access(bundled);
		binary = bundled;
	} catch {
		/* Standalone build. */
	}
}
try {
	await fs.access(binary);
} catch {
	throw new Error(
		"Build Pixel first with npm run build:media, then run npm run demo:media.",
	);
}
console.log(
	`Starting the default show's Pixel companion. Configuration: ${configuration}`,
);
console.log(
	"Open the default show in Control and run Start (Page 1, position 102). Open the standalone Visualizer to see the live walls. Stop with Ctrl+C.",
);
const child = spawn(binary, process.argv.slice(2), {
	stdio: "inherit",
	env: { ...process.env, MEDIA_CONFIG: configuration },
});
for (const signal of ["SIGINT", "SIGTERM"])
	process.on(signal, () => child.kill(signal));
child.on("error", (error) => {
	console.error(error.message);
	process.exitCode = 1;
});
child.on("exit", (code, signal) => {
	process.exitCode = code ?? (signal ? 1 : 0);
});
