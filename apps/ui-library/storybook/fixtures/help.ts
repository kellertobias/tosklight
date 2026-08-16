import type {
  HelpCatalog,
  HelpCatalogEntry,
  HelpTopic,
} from "../../../light-desktop/src/api/types";

export const helpQuickStartId = "00-Quick-Start/index.md";
export const helpNestedTopicId = "10-Desk/20-Programmer-and-Cues/01-command-line.md";

const topics: HelpCatalogEntry[] = [
  { id: helpQuickStartId, title: "Quick Start", kind: "topic", children: [] },
  { id: "10-application-layout.md", title: "Application Layout and Window Manager", kind: "topic", children: [] },
  {
    id: "10-Desk/20-Programmer-and-Cues/index.md",
    title: "Programmer and Cues",
    kind: "folder",
    children: [
      { id: helpNestedTopicId, title: "Command Line", kind: "topic", children: [] },
      { id: "10-Desk/20-Programmer-and-Cues/02-selecting-and-setting-values.md", title: "Selecting and Setting Values", kind: "topic", children: [] },
    ],
  },
  {
    id: null,
    title: "Running a Show",
    kind: "folder",
    children: [
      { id: "40-Running/01-playbacks.md", title: "Playbacks", kind: "topic", children: [] },
    ],
  },
];

export const helpCatalog: HelpCatalog = {
  topics,
  errors: [],
  live: true,
};

export const helpCatalogWarning: HelpCatalog = {
  ...helpCatalog,
  errors: ["One optional help topic could not be indexed."],
};

export const emptyHelpCatalog: HelpCatalog = {
  topics: [],
  errors: [],
  live: false,
};

export const helpQuickStartTopic: HelpTopic = {
  id: helpQuickStartId,
  title: "Quick Start",
  live: true,
  markdown: `# Quick Start

ToskLight is organized around a desk configuration, a portable show file, one programmer per operator, and playbacks that turn stored programming into live output.

![The ToskLight application icon](storybook-quick-start.png)

## Set up the desk

Use [AT], [0-9], [ENT], and [KBD:ENTER] from the desk or computer keyboard. A target such as <selection> can be entered directly.

| Surface | Action |
| --- | --- |
| Desk key | [REC] records the current look |
| Keyboard | [KBD:ESC] clears the current step |

> Save the show before shutting down the desk.

\`\`\`text
Fixture 1 Thru 6 At 50 Please
\`\`\`
`,
};

export const helpNestedTopic: HelpTopic = {
  id: helpNestedTopicId,
  title: "Command Line",
  live: true,
  markdown: `# Command Line

Commands use the same grammar from software, keyboard, OSC, and attached hardware.

## Example

Press [FIX] [1] [THRU] [6] [AT] [5] [0] [ENT].
`,
};
