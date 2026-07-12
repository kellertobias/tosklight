import { describe, expect, it } from "vitest";
import { playbackSlotNumbers } from "./playbackProjection";
describe("playbackSlotNumbers",()=>{it("projects consecutive page slots and preserves holes",()=>{expect(playbackSlotNumbers({number:1,name:"Main",slots:{"20":44,"22":7}},20,4)).toEqual([44,undefined,7,undefined]);});it("supports the slot 127 boundary",()=>{expect(playbackSlotNumbers({number:1,name:"Main",slots:{"127":1000}},127,1)).toEqual([1000]);});});
