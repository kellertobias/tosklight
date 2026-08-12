export function prepareHelpMarkdown(markdown: string): string {
  return markdown
    .replace(/`\[KBD:([^\]\n]+)\]`/g, (_, key: string) => `\`help-keyboard:${key}\``)
    .replace(/\[KBD:([^\]\n]+)\]/g, (_, key: string) => `\`help-keyboard:${key}\``)
    .replace(/`\[\s*([+\-−^.]|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\]`/g, (_, key: string) => `\`help-key:${key.trim()}\``)
    .replace(/`<([a-z][a-z0-9._+-]*\*?)>`/g, (_, placeholder: string) => `\`help-placeholder:${placeholder}\``)
    .replace(/\[\s*([+\-−^.]|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\](?!\()/g, (_, key: string) => `\`help-key:${key.trim()}\``)
    .replace(/<([a-z][a-z0-9._+-]*\*?)>/g, (_, placeholder: string) => `\`help-placeholder:${placeholder}\``);
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
