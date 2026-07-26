declare const artifactResolver: {
  repositoryRoot: string;
  artifactRoot: string;
  artifactPaths: Readonly<{
    root: string;
    cargo: string;
    controlFrontend: string;
    hardwareFrontend: string;
    storybook: string;
    viteCache: string;
    pythonCache: string;
    manual: string;
    release: string;
    runtime: string;
    coverage: string;
    report: string;
    results: string;
    visual: string;
    tmp: string;
  }>;
};
export = artifactResolver;
