import { expect, it } from "vitest";
import type { CueList } from "../../api/types";
import { mergeCueTimingFields } from "./mergeCueTimingFields";
const basis = { id:"list", name:"List", cues:[{id:"cue",name:"First",fade_millis:1000,delay_millis:0,out_fade_millis:200,out_fade_link:"release",trigger:{type:"manual"},changes:[]}] } as unknown as CueList;
it("preserves unrelated authoritative edits received during a pointer gesture", () => {
 const live = {...basis, name:"Renamed list", cues:[{...basis.cues[0], name:"Renamed cue"}]};
 const edit = {...basis, cues:[{...basis.cues[0],fade_millis:2000}]};
 expect(mergeCueTimingFields(live,basis,edit)).toMatchObject({name:"Renamed list",cues:[{name:"Renamed cue",fade_millis:2000}]});
});
it("refuses an intervening edit to the same timing cell", () => {
 const live = {...basis,cues:[{...basis.cues[0],fade_millis:3000}]};
 const edit = {...basis,cues:[{...basis.cues[0],fade_millis:2000}]};
 expect(()=>mergeCueTimingFields(live,basis,edit)).toThrow(/changed during this edit/);
});
it("undo and redo restore the Out timing link as well as its numeric value", () => {
 const edit = {...basis,cues:[{...basis.cues[0],out_fade_millis:500,out_fade_link:undefined}]};
 const stored = mergeCueTimingFields(basis,basis,edit);
 expect(stored.cues[0].out_fade_link).toBeUndefined();
 const undone = mergeCueTimingFields(stored,edit,basis);
 expect(undone.cues[0]).toMatchObject({out_fade_link:"release",out_fade_millis:200});
 expect(mergeCueTimingFields(undone,basis,edit).cues[0].out_fade_link).toBeUndefined();
});
