import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import quickStartImage from "../../../../assets/branding/tosklight-app-icon.png?url";
import {
  emptyHelpCatalog,
  helpCatalog,
  helpCatalogWarning,
  helpNestedTopic,
  helpNestedTopicId,
  helpQuickStartId,
  helpQuickStartTopic,
} from "../../../ui-library/storybook/fixtures/help";
import type { HelpCatalog, HelpTopic } from "../api/types";
import {
  HelpWindowView,
  type HelpWindowViewProps,
  type HelpUrlTransform,
} from "./HelpWindow";

const meta = {
  title: "ToskLight/Windows/Help",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const storyUrlTransform: HelpUrlTransform = (url, kind) => {
  if (kind === "image" && url === "storybook-quick-start.png") return quickStartImage;
  if (url.startsWith("https://")) return url;
  return undefined;
};

function HelpStoryHarness({
  catalog = helpCatalog,
  initialSelected = helpQuickStartId,
  initialTopic = helpQuickStartTopic,
  ...props
}: Partial<HelpWindowViewProps> & {
  initialSelected?: string | null;
  initialTopic?: HelpTopic | null;
  catalog?: HelpCatalog | null;
}) {
  const [selected, setSelected] = useState<string | null>(initialSelected);
  const [topic, setTopic] = useState<HelpTopic | null>(initialTopic);
  const [query, setQuery] = useState(props.query ?? "");
  const select = (id: string) => {
    setSelected(id);
    setTopic(id === helpNestedTopicId ? helpNestedTopic : helpQuickStartTopic);
  };
  return <div style={{ height: props.compact ? 500 : 761, minWidth: 0 }}>
    <HelpWindowView
      catalog={catalog}
      compact={props.compact}
      defaultExpanded={props.defaultExpanded}
      error={props.error}
      loading={props.loading}
      onSelect={select}
      onQueryChange={setQuery}
      query={query}
      selected={selected}
      topic={topic}
      urlTransform={storyUrlTransform}
    />
  </div>;
}

export const QuickStart: Story = {
  render: () => <HelpStoryHarness defaultExpanded={["30-Programmer/index.md", "folder:Running a Show"]} />,
};

export const NestedTopic: Story = {
  render: () => <HelpStoryHarness
    defaultExpanded={["30-Programmer/index.md"]}
    initialSelected={helpNestedTopicId}
    initialTopic={helpNestedTopic}
  />,
};

export const Loading: Story = {
  render: () => <HelpStoryHarness catalog={null} initialSelected={null} initialTopic={null} loading />,
};

export const EmptyCatalog: Story = {
  render: () => <HelpStoryHarness catalog={emptyHelpCatalog} initialSelected={null} initialTopic={null} />,
};

export const CatalogError: Story = {
  render: () => <HelpStoryHarness catalog={null} error="Catalog request failed" initialSelected={null} initialTopic={null} />,
};

export const TopicError: Story = {
  render: () => <HelpStoryHarness error="Topic request failed" initialTopic={null} />,
};

export const CatalogWarning: Story = {
  render: () => <HelpStoryHarness catalog={helpCatalogWarning} />,
};

export const SearchResults: Story = {
  render: () => <HelpStoryHarness
    defaultExpanded={["30-Programmer/index.md"]}
    initialSelected={helpNestedTopicId}
    initialTopic={helpNestedTopic}
    query="Command Line"
  />,
};

export const SearchNoResults: Story = {
  render: () => <HelpStoryHarness query="No such topic" />,
};

export const Compact: Story = {
  render: () => <HelpStoryHarness compact defaultExpanded={["30-Programmer/index.md"]} />,
};
