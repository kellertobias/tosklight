/// <reference types="vite/client" />

export interface IconCatalogItem {
  value: string;
  label: string;
  source: "catalog" | "built-in";
}

export interface IconCatalogGroup {
  id: string;
  label: string;
  icons: readonly IconCatalogItem[];
}

const GROUP_LABELS: Record<string, string> = {
  "beam-size": "Beam size",
  "fixture-base": "Fixture base",
  "fixture-type": "Fixture type",
  flash: "Flash",
  functionality: "Functionality",
  gobo: "Gobo",
  "laser-shape": "Laser shape",
  misc: "Misc",
  position: "Position",
  "position-beam": "Position beam",
  prism: "Prism",
};

const BUILT_IN_ICONS = [
  "⊞", "⌂", "★", "◉", "▶", "▣", "⚙", "◇", "◆", "●", "○", "✦", "☀", "◐", "▰", "⌖",
] as const;

function title(value: string) {
  return value.split("-").map((part) => part ? `${part[0].toUpperCase()}${part.slice(1)}` : "").join(" ");
}

const modules = import.meta.glob<string>("../../../../../assets/icons/**/*.svg", {
  eager: true,
  query: "?url",
  import: "default",
});

const catalogGroups = new Map<string, IconCatalogItem[]>();
for (const [path, url] of Object.entries(modules).sort(([left], [right]) => left.localeCompare(right))) {
  if (path.endsWith(".expanded.svg")) continue;
  const match = /\/assets\/icons\/([^/]+)\/([^/]+)\.svg$/u.exec(path);
  if (!match) continue;
  const [, group, name] = match;
  const icons = catalogGroups.get(group) ?? [];
  icons.push({ value: url, label: title(name), source: "catalog" });
  catalogGroups.set(group, icons);
}

export const ICON_CATALOG_GROUPS: readonly IconCatalogGroup[] = [
  {
    id: "built-in",
    label: "Built-in / General",
    icons: BUILT_IN_ICONS.map((value) => ({ value, label: `Built-in ${value}`, source: "built-in" as const })),
  },
  ...Object.entries(GROUP_LABELS).map(([id, label]) => ({
    id,
    label,
    icons: catalogGroups.get(id) ?? [],
  })),
];

export function resolveIconGroup(requested?: string) {
  return ICON_CATALOG_GROUPS.some((group) => group.id === requested)
    ? requested!
    : "built-in";
}

export function iconCatalogItem(value: string) {
  return ICON_CATALOG_GROUPS.flatMap((group) => group.icons).find((icon) => icon.value === value);
}
