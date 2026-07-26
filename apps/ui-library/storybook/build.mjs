import { spawn } from "node:child_process";
import { mkdir } from "node:fs/promises";
import resolver from "../../../tools/artifact-paths.cjs";

const { artifactPaths, repositoryRoot } = resolver;
await mkdir(artifactPaths.storybook, { recursive: true });

const storybookCli = new URL(
  "../../../node_modules/storybook/dist/bin/dispatcher.js",
  import.meta.url,
);
const child = spawn(
  process.execPath,
  [
    storybookCli.pathname,
    "build",
    "--config-dir",
    "storybook/config",
    "--output-dir",
    artifactPaths.storybook,
  ],
  {
    cwd: new URL("..", import.meta.url),
    stdio: "inherit",
    env: process.env,
  },
);

child.on("error", (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on("exit", (code, signal) => {
  if (signal) {
    console.error(`Storybook build terminated by ${signal}`);
    process.exitCode = 1;
  } else {
    process.exitCode = code ?? 1;
  }
});
