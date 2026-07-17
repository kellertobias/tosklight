export const oscPaths = {
  page: "page",
  pagePlayback: (slot: number) => `page-playback/${slot}`,
  programmer: (action: string) => `programmer/${action}`,
  highlight: (action: "on" | "off" | "toggle" | "capture" | "next" | "previous") => `highlight/${action}`,
  speedGroupButton: (group: number) => `speed-group/${group}/button`,
  speedGroupEncoder: (group: number) => `speed-group/${group}/encoder`,
  encoder: (number: number) => `encode/${number}`,
  navigation: "nav",
} as const;

export function feedbackPagePlaybackOffset(parts: string[]): number {
  const canonical = parts.indexOf("page-playback");
  return canonical >= 0 ? canonical : parts.indexOf("paged-playback");
}
