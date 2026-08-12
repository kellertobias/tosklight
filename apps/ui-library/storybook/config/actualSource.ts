/// <reference types="vite/client" />

type StoryContext = {
  parameters?: {
    fileName?: string;
  };
};

type SourceModule = {
  path: string;
  code: string;
};

const rawModules = import.meta.glob(
  ["../../src/**/*.tsx", "../../../light-desktop/src/**/*.tsx"],
  { eager: true, import: "default", query: "?raw" },
) as Record<string, string>;

function repositoryPath(globPath: string): string {
  if (globPath.startsWith("../../src/")) {
    return `apps/ui-library/src/${globPath.slice("../../src/".length)}`;
  }
  if (globPath.startsWith("../../../light-desktop/src/")) {
    return `apps/light-desktop/src/${globPath.slice("../../../light-desktop/src/".length)}`;
  }
  return globPath;
}

const sourceByPath = new Map(
  Object.entries(rawModules).map(([path, code]) => [repositoryPath(path), code]),
);

const exceptionalImplementations: Record<string, string[]> = {
  "apps/ui-library/src/common/Buttons.stories.tsx": [
    "apps/ui-library/src/common/controls/foundation.tsx",
  ],
  "apps/ui-library/src/encoders/EncoderSurfaces.stories.tsx": [
    "apps/ui-library/src/encoders/EncoderSection.tsx",
    "apps/ui-library/src/encoders/TouchEncoder.tsx",
    "apps/ui-library/src/encoders/HardwareEncoderDisplay.tsx",
    "apps/ui-library/src/encoders/EncoderGroupTabs.tsx",
  ],
  "apps/ui-library/src/input/InputSurfaces.stories.tsx": [
    "apps/ui-library/src/input/ModalInputControls.tsx",
    "apps/ui-library/src/input/ModalNumberEditor.tsx",
  ],
  "apps/ui-library/src/tables/DataTable.stories.tsx": [
    "apps/ui-library/src/window-kit/WindowKit.tsx",
  ],
  "apps/light-desktop/src/MarketingScreenshots.stories.tsx": [
    "apps/light-desktop/src/MarketingScreenshots.stories.tsx",
  ],
  "apps/light-desktop/src/components/ApplicationShell.stories.tsx": [
    "apps/light-desktop/src/components/ApplicationShell.stories.tsx",
    "apps/light-desktop/src/components/shell/AppShell.tsx",
  ],
  "apps/light-desktop/src/components/modals/ApplicationModalWorkflows.stories.tsx": [
    "apps/light-desktop/src/components/modals/ApplicationModalWorkflows.stories.tsx",
    "apps/light-desktop/src/components/control/PlaybackConfigurationModal.tsx",
    "apps/light-desktop/src/components/shared/RecordModeDialog.tsx",
  ],
  "apps/light-desktop/src/windows/PoolWindows.stories.tsx": [
    "apps/light-desktop/src/windows/PoolWindows.stories.tsx",
    "apps/light-desktop/src/windows/GroupsWindow.tsx",
    "apps/light-desktop/src/windows/groupsWindow/GroupPoolGrid.tsx",
    "apps/light-desktop/src/windows/presetsWindow/PresetsWindowView.tsx",
  ],
  "apps/light-desktop/src/windows/TimecodeWindow.stories.tsx": [
    "apps/light-desktop/src/windows/TimecodeWindow.stories.tsx",
    "apps/light-desktop/src/windows/stories/TimecodeWindow.tsx",
    "apps/light-desktop/src/features/timecode/TimecodeTimelineEditor.tsx",
  ],
};

function normalizeStoryPath(fileName: string | undefined): string | null {
  if (!fileName) return null;
  const normalized = fileName.replaceAll("\\", "/");
  const appsIndex = normalized.lastIndexOf("apps/");
  if (appsIndex >= 0) return normalized.slice(appsIndex);
  if (normalized.startsWith("./src/")) {
    return `apps/ui-library/src/${normalized.slice("./src/".length)}`;
  }
  if (normalized.startsWith("../light-desktop/src/")) {
    return `apps/light-desktop/src/${normalized.slice("../light-desktop/src/".length)}`;
  }
  return normalized.replace(/^\.\//, "");
}

function implementationPaths(storyPath: string): string[] {
  const exceptional = exceptionalImplementations[storyPath];
  if (exceptional) return exceptional;

  const sibling = storyPath.replace(/\.stories\.tsx$/, ".tsx");
  if (sourceByPath.has(sibling)) return [sibling];

  return sourceByPath.has(storyPath) ? [storyPath] : [];
}

function sourceModules(storyPath: string): SourceModule[] {
  return implementationPaths(storyPath).flatMap((path) => {
    const code = sourceByPath.get(path);
    return code == null ? [] : [{ path, code }];
  });
}

function formatSource(storySource: string, modules: SourceModule[]): string {
  const sections = [
    `// Story usage\n${storySource.trim()}`,
    ...modules.map(({ path, code }) => `// Actual source: ${path}\n${code.trim()}`),
  ];
  return sections.join("\n\n");
}

export function actualSourceForStory(storySource: string, context: StoryContext): string {
  const storyPath = normalizeStoryPath(context.parameters?.fileName);
  if (!storyPath) return storySource;
  const modules = sourceModules(storyPath);
  return modules.length > 0 ? formatSource(storySource, modules) : storySource;
}

export const actualSourceCoverage = [...sourceByPath.keys()]
  .filter((path) => path.endsWith(".stories.tsx"))
  .sort()
  .map((storyPath) => ({
    storyPath,
    implementationPaths: implementationPaths(storyPath),
    missingImplementationPaths: implementationPaths(storyPath).filter(
      (path) => !sourceByPath.has(path),
    ),
  }));
