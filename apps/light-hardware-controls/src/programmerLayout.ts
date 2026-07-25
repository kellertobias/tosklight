export const attachedHighlightKeys = [
  { label: "HIGH", action: "toggle", column: 1, row: 1 },
  { label: "PREV", action: "previous", column: 2, row: 1 },
  { label: "NEXT", action: "next", column: 3, row: 1 },
  { label: "ALL", action: "all", column: 4, row: 1 },
] as const;

export const attachedProgrammerActionLayout = {
  record: { column: 1, row: 1, rowSpan: 2 },
  preload: { column: 2, row: 1, rowSpan: 2 },
} as const;

export const attachedKeypadContentRowOffset = 1;
