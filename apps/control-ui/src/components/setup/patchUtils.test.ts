import { describe, expect, it } from "vitest";
import type { FixtureDefinition, PatchedFixture } from "../../api/types";
import { conflicts, firstFreeAddress, groupFixtureFamilies, incrementFixtureName } from "./patchUtils";
import { parsePatchAddress } from "../input/ConsoleFields";

const definition = (footprint = 10, mode = "Standard"): FixtureDefinition => ({ schema_version:1,id:mode,revision:1,manufacturer:"Acme",device_type:"wash mover",name:"Beam",model:"Beam",mode,footprint,heads:[],color_calibration:null,physical:{},hazardous:false,direct_control_protocols:[],signal_loss_policy:{type:"hold_last"},safe_values:{} });
const fixture = (id:string,address:number|null,footprint=10): PatchedFixture => ({fixture_id:id,name:id,universe:address == null ? null : 1,address,layer_id:"default",definition:definition(footprint),logical_heads:[]});

describe("patch utilities", () => {
  it("parses the single universe.address field", () => { expect(parsePatchAddress("2.101")).toEqual({universe:2,address:101}); expect(parsePatchAddress("2")).toBeNull(); expect(parsePatchAddress("1.513")).toBeNull(); });
  it("increments trailing fixture numbers", () => { expect(incrementFixtureName("Wash 07",2)).toBe("Wash 9"); expect(incrementFixtureName("Wash",1)).toBe("Wash 2"); });
  it("ignores unpatched fixtures and finds exact free boundaries", () => { const fixtures=[fixture("one",1),fixture("unpatched",null),fixture("two",21)]; expect(conflicts(fixtures,1,10,2)).toHaveLength(1); expect(firstFreeAddress(fixtures,1,10)).toBe(11); expect(firstFreeAddress([fixture("edge",503,10)],1,10,503)).toBeNull(); });
  it("groups named modes into a fixture family", () => { const families=groupFixtureFamilies([definition(10,"A"),definition(20,"B")]); expect(families).toHaveLength(1); expect(families[0].modes.map((mode)=>mode.mode)).toEqual(["A","B"]); });
});
