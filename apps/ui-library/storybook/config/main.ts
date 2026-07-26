import type { StorybookConfig } from "@storybook/react-vite";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "../../../../tools/artifact-paths.mjs";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));

const config: StorybookConfig = {
  stories: [
    "../../src/**/*.stories.@(ts|tsx)",
    "../../../light-desktop/src/**/*.stories.@(ts|tsx)",
  ],
  addons: [
    "@storybook/addon-docs",
    "@storybook/addon-a11y",
    "@storybook/addon-vitest",
  ],
  framework: {
    name: "@storybook/react-vite",
    options: {},
  },
  docs: { autodocs: "tag" },
  core: { disableTelemetry: true },
  viteFinal: async (viteConfig) => ({
    ...viteConfig,
    cacheDir: `${artifactPaths.viteCache}/ui-library-storybook`,
    resolve: {
      ...viteConfig.resolve,
      dedupe: ["react", "react-dom"],
    },
    server: {
      ...viteConfig.server,
      fs: {
        ...viteConfig.server?.fs,
        allow: [repositoryRoot],
      },
    },
  }),
};

export default config;
