import { describe, expect, it, vi } from "vitest";
import type { FixtureDefinition } from "../../../api/types";
import { saveEdit } from "./editSave";
import type { PatchController } from "./controller";
import { rootProgrammingCorrespondences, rootProgrammingDecision } from "./replacementProgramming";

function definition(shared: boolean, names: string[], attribute = "intensity"): FixtureDefinition {
	const heads = names.map((name,index) => ({id:`head-${shared ? "old" : "new"}-${index}`,name,master_shared:shared}));
	return {
		mode_id:"mode", heads:heads.map(() => ({parameters:[]})),
		profile_snapshot:{id:"profile",revision:7,modes:[{id:"mode",heads,channels:heads.map(head => ({head_id:head.id,behavior:"controlled",attribute,fixture_attribute:attribute,functions:[]}))}]},
	} as unknown as FixtureDefinition;
}

describe("explicit existing shared-root programming correspondence", () => {
	it("offers two capability-compatible owners and never chooses them automatically", () => {
		const rows = rootProgrammingCorrespondences(definition(true,["Shared"]),definition(false,["Left","Right"]));
		expect(rows).toHaveLength(1);
		expect(rows[0].targets.map(target => target.id)).toEqual(["head-new-0","head-new-1"]);
		expect(rootProgrammingDecision(rows[0],undefined)).toBeNull();
		expect(rootProgrammingDecision(rows[0],"")).toBeNull();
		expect(rootProgrammingDecision(rows[0],"head-new-0,head-new-1")?.targetProfileHeadIds).toEqual(["head-new-0","head-new-1"]);
	});
	it("distinguishes explicit dormant intent from missing consent and rejects foreign/duplicate owners", () => {
		const [row] = rootProgrammingCorrespondences(definition(true,["Shared"]),definition(false,["Only"]));
		expect(rootProgrammingDecision(row,"__unmapped")?.targetProfileHeadIds).toEqual([]);
		expect(rootProgrammingDecision(row,"foreign-head")).toBeNull();
		expect(rootProgrammingDecision(row,"head-new-0,head-new-0")).toBeNull();
	});
	it("offers no incompatible family and keeps ordinary nonshared replacement rows separate", () => {
		const [row] = rootProgrammingCorrespondences(definition(true,["Shared"]),definition(false,["Color only"],"color.red"));
		expect(row.targets).toEqual([]);
		expect(rootProgrammingDecision(row,"__unmapped")?.targetProfileHeadIds).toEqual([]);
		expect(rootProgrammingCorrespondences(definition(false,["Old child"]),definition(false,["New child"]))).toEqual([]);
	});
	it("uses semantic color/position family correspondence without selecting channel slots", () => {
		const [row] = rootProgrammingCorrespondences(definition(true,["Pan root"],"pan"),definition(false,["Tilt head"],"tilt"));
		expect(row.attribute).toBe("position");
		expect(row.targets[0].id).toBe("head-new-0");
	});
});


describe("replacement Save control", () => {
    function controller(choice?: string) {
        const source = definition(true,["Shared"]);
        const target = definition(false,["Left","Right"]);
        const [row] = rootProgrammingCorrespondences(source,target);
        const send = vi.fn(async () => false);
        const error = vi.fn();
        const control = {
            data:{selected:{fixture_id:"root",definition:source,logical_heads:[]},all:[],definition:target},
            ui:{edit:"mode",vector:{},editAxis:"x",replacingFixture:true,replacementRevision:{show:3,patch:2},replacementHeads:choice ? {[row.key]:choice} : {},setEditError:error},
            patch:{updateFixtureIntent:send,error:""},
        } as unknown as PatchController;
        return {control,send,error};
    }
    it("blocks Save until every shared-source family has an explicit decision", () => {
        const {control,send,error} = controller();
        saveEdit(control);
        expect(send).not.toHaveBeenCalled();
        expect(error).toHaveBeenCalledWith(expect.stringContaining("Choose replacement owners"));
    });
    it.each([ ["head-new-0,head-new-1",["head-new-0","head-new-1"]], ["__unmapped",[]] ] as const)("sends the exact consent %s with captured revisions", (choice,targets) => {
        const {control,send} = controller(choice);
        saveEdit(control);
        expect(send).toHaveBeenCalledWith("root",null,expect.objectContaining({type:"replace_profile",expectedShowRevision:3,expectedPatchRevision:2,rootProgrammingMapping:[{sourceProfileHeadId:"head-old-0",attribute:"intensity",targetProfileHeadIds:targets}]}));
    });
});
