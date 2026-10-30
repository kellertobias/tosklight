/**
 * Which command lines change stored show objects, and so need the caller's explicit confirmation.
 *
 * The desk itself never asks: an operator who types DELETE GROUP 9 means it. A model driving the
 * desk is a different author, so a line that deletes, moves, copies over, updates, or records over
 * something that already exists is refused unless the call says `confirm: true`. Recording into a
 * new Cue, Group or Preset, selecting, and setting Programmer values are never held back.
 */

export interface StoredObjects {
	/** Cue numbers per Cuelist pool number. */
	cues: ReadonlyMap<number, ReadonlySet<string>>;
	groups: ReadonlySet<string>;
	/** `family:number`, e.g. `color:3`. */
	presets: ReadonlySet<string>;
}

export interface CommandRisk {
	destructive: boolean;
	reason: string | null;
}

const FAMILIES = ["intensity", "color", "position", "beam", "focus"];

function words(command: string) {
	return command.trim().toUpperCase().split(/\s+/).filter(Boolean);
}

/** Why a command would overwrite or remove stored programming, or null when it would not. */
export function commandRisk(command: string, stored: StoredObjects): CommandRisk {
	const tokens = words(command);
	const verbAt = tokens.findIndex((token) =>
		["DELETE", "DEL", "MOVE", "MOV", "COPY", "CPY", "UPDATE", "RECORD", "REC"].includes(token),
	);
	if (verbAt < 0) return { destructive: false, reason: null };
	const verb = tokens[verbAt];
	if (verb === "DELETE" || verb === "DEL") return { destructive: true, reason: "it deletes stored programming" };
	if (verb === "MOVE" || verb === "MOV") return { destructive: true, reason: "it moves stored programming" };
	if (verb === "COPY" || verb === "CPY") return { destructive: true, reason: "a copy can replace the destination" };
	if (verb === "UPDATE") return { destructive: true, reason: "it changes stored programming" };
	const target = tokens.slice(verbAt + 1).filter((token) => token !== "+" && token !== "-");
	const merge = tokens.slice(verbAt + 1).some((token) => token === "+" || token === "-");
	const number = (index: number) => target[index] ?? null;
	// RECORD CUELIST <list> [CUE <cue>] — appending is new; an existing Cue number is an overwrite.
	if (target[0] === "CUELIST" || (target[0] === "CUE" && target[1] === "CUE")) {
		const at = target[0] === "CUELIST" ? 1 : 2;
		const list = Number(number(at));
		const cueAt = target.indexOf("CUE", at);
		if (cueAt < 0) return { destructive: false, reason: null };
		const cue = number(cueAt + 1);
		if (cue && stored.cues.get(list)?.has(cue))
			return { destructive: true, reason: `Cue ${cue} in Cuelist ${list} already exists` };
		return { destructive: false, reason: null };
	}
	if (target[0] === "GROUP") {
		const group = number(1);
		if (group && stored.groups.has(group))
			return { destructive: true, reason: `Group ${group} already exists${merge ? " and would be merged into" : ""}` };
		return { destructive: false, reason: null };
	}
	if (target[1] === "PRESET" && FAMILIES.includes(target[0].toLowerCase())) {
		const key = `${target[0].toLowerCase()}:${number(2)}`;
		if (stored.presets.has(key))
			return { destructive: true, reason: `${target[0]} Preset ${number(2)} already exists` };
		return { destructive: false, reason: null };
	}
	// A Playback or a selected-Cuelist record appends; anything else we cannot prove is new.
	if (target[0] === "PBK" || target.length === 0 || (target[0] === "CUE" && target.length === 1))
		return { destructive: false, reason: null };
	return { destructive: true, reason: "its target may already exist" };
}
