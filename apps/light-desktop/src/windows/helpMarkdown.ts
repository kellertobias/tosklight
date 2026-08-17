export function prepareHelpMarkdown(markdown: string): string {
  return markdown
    .replace(/`\[KBD:([^\]\n]+)\]`/g, (_, key: string) => `\`help-keyboard:${key}\``)
    .replace(/\[KBD:([^\]\n]+)\]/g, (_, key: string) => `\`help-keyboard:${key}\``)
    .replace(/`\[\s*([+\-−^.]|\^[A-Z0-9.][A-Z0-9._ ←-]*|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\]`/g, (_, key: string) => `\`help-key:${key.trim()}\``)
    .replace(/`<([a-z][a-z0-9._+-]*\*?)>`/g, (_, placeholder: string) => `\`help-placeholder:${placeholder}\``)
    .replace(/\[\s*([+\-−^.]|\^[A-Z0-9.][A-Z0-9._ ←-]*|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\](?!\()/g, (_, key: string) => `\`help-key:${key.trim()}\``)
    .replace(/<([a-z][a-z0-9._+-]*\*?)>/g, (_, placeholder: string) => `\`help-placeholder:${placeholder}\``);
}

interface HelpMarkdownNode {
  type: string;
  value?: string;
  children?: HelpMarkdownNode[];
  data?: {
    hName?: string;
    hProperties?: Record<string, unknown>;
  };
}

function calloutTitle(type: string): string {
  return type.charAt(0).toLocaleUpperCase() + type.slice(1);
}

/** Render Obsidian `> [!type] Title` blocks as semantic callouts. */
export function remarkObsidianCallouts() {
  return (tree: HelpMarkdownNode) => {
    const visit = (node: HelpMarkdownNode) => {
      for (const child of node.children ?? []) {
        if (child.type === "blockquote") {
          const firstParagraph = child.children?.[0];
          const firstText = firstParagraph?.type === "paragraph"
            ? firstParagraph.children?.[0]
            : undefined;
          const match = firstText?.type === "text"
            ? /^\[!([a-z0-9_-]+)\]([+-])?(?:[ \t]+([^\n]+))?(?:\n|$)/i.exec(firstText.value ?? "")
            : null;
          if (match && firstParagraph && firstText) {
            const type = match[1].toLocaleLowerCase();
            const title = match[3]?.trim() || calloutTitle(type);
            const remainder = (firstText.value ?? "").slice(match[0].length);
            firstParagraph.children = [
              { type: "strong", children: [{ type: "text", value: title }] },
              ...(remainder ? [{ type: "text", value: remainder }] : []),
              ...(firstParagraph.children?.slice(1) ?? []),
            ];
            child.data = {
              ...child.data,
              hName: "aside",
              hProperties: {
                className: ["help-callout", `help-callout-${type}`],
                "data-callout": type,
                "data-callout-fold": match[2] || undefined,
              },
            };
          }
        }
        visit(child);
      }
    };
    visit(tree);
  };
}

function resolveHelpPath(url: string, topicId?: string): string | undefined {
  const parts = topicId ? topicId.split("/").slice(0, -1) : [];
  for (const part of url.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      if (parts.length === 0) return undefined;
      parts.pop();
      continue;
    }
    parts.push(part);
  }
  return parts.length ? parts.join("/") : undefined;
}

export function safeHelpUrl(
  url: string,
  kind: "link" | "image",
  topicId?: string,
): string | undefined {
  const trimmed = url.trim();
  if (/^https:\/\//i.test(trimmed)) return trimmed;
  if (kind === "link" && trimmed.startsWith("#")) return trimmed;
  if (
    kind === "link" &&
    !/^[a-z][a-z0-9+.-]*:/i.test(trimmed) &&
    !trimmed.startsWith("/")
  ) {
    const [target] = trimmed.split("#", 1);
    const path = resolveHelpPath(target, topicId);
    if (path) return `#help-topic:${encodeURIComponent(path)}`;
  }
  if (
    kind === "image" &&
    !/^[a-z][a-z0-9+.-]*:/i.test(trimmed) &&
    !trimmed.startsWith("/")
  ) {
    const path = resolveHelpPath(trimmed, topicId);
    if (path)
      return `/api/v2/help/assets/${path.split("/").map(encodeURIComponent).join("/")}`;
  }
  return undefined;
}
