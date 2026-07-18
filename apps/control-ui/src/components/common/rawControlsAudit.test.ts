import { describe, expect, it } from "vitest";

const sources=import.meta.glob("/src/**/*.tsx",{eager:true,query:"?raw",import:"default"}) as Record<string,string>;
describe("shared-control enforcement",()=>{it("keeps raw controls inside the shared primitive implementation",()=>{const offenders=Object.entries(sources).filter(([file,source])=>!file.endsWith("/components/common/controls.tsx")&&!file.endsWith("/components/common/ModalTitleBar.tsx")&&/<(?:button|input|select|textarea)\b/.test(source)).map(([file])=>file);expect(offenders).toEqual([]);});});

describe("unified form enforcement",()=>{it("keeps ordinary fields out of the legacy Input compatibility wrapper",()=>{const offenders=Object.entries(sources).filter(([file])=>!file.endsWith("/components/control/commandLine/CommandInput.tsx")&&!file.endsWith("/components/common/controls.tsx")).filter(([,source])=>[...source.matchAll(/<Input\b[^>]*>/g)].some(([tag])=>!/(?:type="(?:file|range|hidden)"|hidden)/.test(tag))).map(([file])=>file);expect(offenders).toEqual([]);});});
