import type { DiscoveredMediaOutput } from "../../api/client/mediaOutput";
import type { FixtureDefinition, PatchedFixture } from "../../api/types";

const PROTOCOL_LABELS: Record<string, string> = {
	"art-net": "Art-Net",
	sacn: "sACN",
};

/** The discovered output's current Media Server configuration as the operator reads it. */
export function discoveredOutputFacts(output: DiscoveredMediaOutput): string {
	const facts = [
		`Suggested DMX ${output.universe}.${output.startAddress}`,
		output.mode ?? "Unsupported personality",
		PROTOCOL_LABELS[output.protocol] ?? output.protocol,
	];
	const tempo = tempoLabel(output);
	if (tempo) facts.push(tempo);
	return facts.filter(Boolean).join(" · ");
}

function tempoLabel(output: DiscoveredMediaOutput): string | null {
	if (output.tempoSource === "speed-group")
		return output.speedGroup
			? `Tempo from Speed Group ${output.speedGroup}`
			: "Tempo from a Speed Group";
	if (output.tempoSource === "playback-bpm-channel")
		return "Tempo from Playback BPM";
	return null;
}

/** The shipped ToskLight Media Server profile mode this output is patched with. */
export function mediaServerDefinition(
	library: readonly FixtureDefinition[],
	mode: string,
): FixtureDefinition | undefined {
	return library.find(
		(candidate) =>
			candidate.manufacturer === "ToskLight" &&
			candidate.name === "Media Server" &&
			candidate.mode === mode,
	);
}

/**
 * When the desk patch names a different Media Server mode than the output's personality, the
 * operator must know before controlling it: every layer after the first would be misaddressed.
 */
export function patchedModeMismatch(
	fixture: PatchedFixture | undefined,
	output: DiscoveredMediaOutput,
): string | null {
	if (!fixture || !output.mode) return null;
	const definition = fixture.definition;
	if (
		definition.manufacturer !== "ToskLight" ||
		definition.name !== "Media Server" ||
		definition.mode === output.mode
	)
		return null;
	return `The desk patch uses ${definition.mode}, but this output uses ${output.mode}. Patch suggested switches the desk fixture to ${output.mode}.`;
}
