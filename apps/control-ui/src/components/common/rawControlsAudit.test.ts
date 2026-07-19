import { describe, expect, it } from "vitest";

const sources = import.meta.glob("/src/**/*.tsx", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

function ownsRawControls(file: string) {
  return file.includes("/components/common/controls/")
    || file.endsWith("/components/common/ModalTitleBar.tsx");
}

function ownsLegacyInputUsage(file: string) {
  return file.endsWith("/components/control/commandLine/CommandInput.tsx")
    || file.includes("/components/common/controls/");
}

describe("shared-control enforcement", () => {
  it("keeps raw controls inside the shared primitive implementation", () => {
    const offenders = Object.entries(sources)
      .filter(([file, source]) => !ownsRawControls(file)
        && /<(?:button|input|select|textarea)\b/.test(source))
      .map(([file]) => file);

    expect(offenders).toEqual([]);
  });
});

describe("unified form enforcement", () => {
  it("keeps ordinary fields out of the legacy Input compatibility wrapper", () => {
    const offenders = Object.entries(sources)
      .filter(([file]) => !ownsLegacyInputUsage(file))
      .filter(([, source]) => [...source.matchAll(/<Input\b[^>]*>/g)]
        .some(([tag]) => !/(?:type="(?:file|range|hidden)"|hidden)/.test(tag)))
      .map(([file]) => file);

    expect(offenders).toEqual([]);
  });
});
