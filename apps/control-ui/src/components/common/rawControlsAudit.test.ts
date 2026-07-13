import { describe, expect, it } from "vitest";

const sources=import.meta.glob("/src/**/*.tsx",{eager:true,query:"?raw",import:"default"}) as Record<string,string>;
describe("shared-control enforcement",()=>{it("keeps raw controls inside the shared primitive implementation",()=>{const offenders=Object.entries(sources).filter(([file,source])=>!file.endsWith("/components/common/controls.tsx")&&/<(?:button|input|select|textarea)\b/.test(source)).map(([file])=>file);expect(offenders).toEqual([]);});});
