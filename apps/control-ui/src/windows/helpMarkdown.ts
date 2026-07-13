export function prepareHelpMarkdown(markdown: string): string {
  return markdown
    .replace(/`\[([+]|[A-Z0-9.][A-Z0-9._ -]*[+*]?)\]`/g, (_, key: string) => `\`help-key:${key}\``)
    .replace(/`<([a-z][a-z0-9._+-]*\*?)>`/g, (_, placeholder: string) => `\`help-placeholder:${placeholder}\``)
    .replace(/\[([+]|[A-Z0-9.][A-Z0-9._ -]*[+*]?)\]/g, (_, key: string) => `\`help-key:${key}\``)
    .replace(/<([a-z][a-z0-9._+-]*\*?)>/g, (_, placeholder: string) => `\`help-placeholder:${placeholder}\``);
}

export function safeHelpUrl(url: string, kind: "link" | "image"): string | undefined {
  const trimmed = url.trim();
  if (/^https:\/\//i.test(trimmed)) return trimmed;
  if (kind === "link" && trimmed.startsWith("#")) return trimmed;
  if (kind === "image" && !/^[a-z][a-z0-9+.-]*:/i.test(trimmed) && !trimmed.startsWith("/") && !trimmed.split("/").includes("..")) {
    return `/api/v1/help/assets/${trimmed.split("/").map(encodeURIComponent).join("/")}`;
  }
  return undefined;
}
