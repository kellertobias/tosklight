function keyVariant(label, fallback = "regular") {
  if (label === "REC") return "record";
  if (label === "CLR") return "clear";
  if (label === "PRELD") return "preload";
  return fallback;
}

function deskKeys(value) {
  const shifted = value.startsWith("^");
  const label = shifted ? value.slice(1) : value;
  return shifted
    ? [
        { label: "", icon: "shift", variant: "shift" },
        ...(label ? [{ label, variant: keyVariant(label) }] : []),
      ]
    : [{
        label: label === "<--" ? "" : label,
        ...(label === "<--" ? { icon: "backspace" } : {}),
        variant: keyVariant(label),
      }];
}

function controlSegments(source) {
  const pattern = /\[\s*(KBD:[^\]\n]+|\^(?:[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)?|<--|[+\-−.]|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\]/gu;
  const segments = [];
  let cursor = 0;
  for (const match of source.matchAll(pattern)) {
    if ((match.index ?? 0) > cursor) segments.push({ text: source.slice(cursor, match.index) });
    const value = match[1];
    segments.push(value.startsWith("KBD:")
      ? { keys: [{ label: value.slice(4), variant: "keyboard" }] }
      : { keys: deskKeys(value) });
    cursor = (match.index ?? 0) + match[0].length;
  }
  if (cursor < source.length) segments.push({ text: source.slice(cursor) });
  return segments.some((segment) => segment.keys) ? segments : undefined;
}

export default {
  name: "tosklight-control-keys",
  transform(node) {
    if (node.type === "inlineCode") {
      const source = String(node.value ?? "");
      if (source.trimStart().startsWith("#>")) {
        return {
          ...node,
          value: source.trimStart().replace(/^#>\s*/u, ""),
          data: { ...node.data, presentation: { component: "command-line" } },
        };
      }
      const segments = controlSegments(source);
      if (segments) return {
        ...node,
        data: { ...node.data, presentation: { component: "control-sequence", segments } },
      };
    }
    if (node.type !== "inlineToken") return undefined;
    const kind = node.data?.kind;
    const value = String(node.value ?? "").trim();
    if (kind === "keyboard") {
      return {
        ...node,
        data: {
          ...node.data,
          presentation: { component: "key-sequence", keys: [{ label: value, variant: "keyboard" }] },
        },
      };
    }
    if (kind !== "desk-key") return undefined;
    return {
      ...node,
      data: { ...node.data, presentation: { component: "key-sequence", keys: deskKeys(value) } },
    };
  },
};
