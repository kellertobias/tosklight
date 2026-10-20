import { expect, it } from "vitest";
import { playbackTileActionLabel } from "./playbackTileActionLabel";

it.each([
	["1-go", "GO +", "SOLO · GO +"],
	["0-toggle", "TOGGLE", "SOLO · TOGGLE"],
	["2-on", "ON", "SOLO · ON"],
	["1-temp", "TEMP", "SOLO · TEMP"],
	["0-flash", "FLASH", "FLASH"],
	["0-swap", "SWAP", "SWAP"],
	["1-off", "OFF", "OFF"],
	["0-go_minus", "GO −", "GO −"],
])("labels saved Solo member action %s as %s without replacing its actual operation", (id, label, expected) => {
	expect(playbackTileActionLabel({ id, label }, true)).toBe(expected);
	expect(playbackTileActionLabel({ id, label }, false)).toBe(label);
});

it("does not invent an action label for an unassigned tile", () => {
	expect(playbackTileActionLabel(undefined, true)).toBeUndefined();
});
