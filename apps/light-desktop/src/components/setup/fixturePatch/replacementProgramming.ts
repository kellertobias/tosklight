import type { FixtureDefinition, FixtureMode } from "../../../api/types";

export interface RootProgrammingCorrespondence {
	sourceProfileHeadId: string;
	sourceName: string;
	attribute: string;
	key: string;
	targets: Array<{id: string; name: string}>;
}

export function replacementMode(definition: FixtureDefinition | undefined): FixtureMode | undefined {
	return definition?.profile_snapshot?.modes.find(mode => mode.id === definition.mode_id);
}

function canonicalFamily(attribute: string): string {
	if (attribute.startsWith("color.")) return "color";
	if (attribute === "pan" || attribute === "tilt" || attribute.startsWith("position.")) return "position";
	return attribute;
}

function attributes(definition: FixtureDefinition, mode: FixtureMode, headId: string): Set<string> {
	const values = new Set<string>();
	for (const channel of mode.channels.filter(channel => channel.head_id === headId && channel.behavior !== "static")) {
		values.add(canonicalFamily(channel.attribute));
		values.add(canonicalFamily(channel.fixture_attribute));
		for (const fn of channel.functions) values.add(canonicalFamily(fn.attribute));
	}
	if (mode.color_physical?.paths.some(path => path.head_id === headId && path.controls.length > 0)) values.add("color");
	const index = mode.heads.findIndex(head => head.id === headId);
	if (definition.heads[index]?.parameters.some(parameter => parameter.attribute === "intensity" && parameter.virtual_dimmer)) values.add("intensity");
	values.delete("");
	return values;
}

export function rootProgrammingCorrespondences(
	source: FixtureDefinition | undefined,
	target: FixtureDefinition | undefined,
): RootProgrammingCorrespondence[] {
	const oldMode = replacementMode(source);
	const newMode = replacementMode(target);
	if (!source || !target || !oldMode || !newMode) return [];
	return oldMode.heads.filter(head => head.master_shared).flatMap(head =>
		[...attributes(source, oldMode, head.id)].sort().map(attribute => ({
			sourceProfileHeadId: head.id, sourceName: head.name || "Shared root", attribute,
			key: `root:${head.id}:${attribute}`,
			targets: newMode.heads.filter(head => attributes(target, newMode, head.id).has(attribute)).map((head,index) => ({
				id: head.id, name: `${head.name || "Head"} · ${index + 1}${head.master_shared ? " (master)" : ""}`,
			})),
		})),
	);
}

/** Empty text is no decision; __unmapped is deliberate dormancy, not automatic correspondence. */
export function rootProgrammingDecision(row: RootProgrammingCorrespondence, text: string | undefined) {
	if (!text) return null;
	const ids = text === "__unmapped" ? [] : text.split(",");
	if (new Set(ids).size !== ids.length || ids.some(id => !row.targets.some(target => target.id === id))) return null;
	return {sourceProfileHeadId: row.sourceProfileHeadId, attribute: row.attribute, targetProfileHeadIds: ids};
}
