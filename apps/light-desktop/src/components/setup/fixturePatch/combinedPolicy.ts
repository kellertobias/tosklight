export type CombinedPolicyChoice = "none" | "first" | "second" | "both";

export function combinedPolicyChoice(
	first: boolean,
	second: boolean,
): CombinedPolicyChoice {
	if (first && second) return "both";
	if (first) return "first";
	if (second) return "second";
	return "none";
}

export function combinedPolicyValues(choice: CombinedPolicyChoice) {
	return {
		first: choice === "first" || choice === "both",
		second: choice === "second" || choice === "both",
	};
}

export function allowedCombinedPolicyChoices(
	firstAvailable: boolean,
	secondAvailable: boolean,
): CombinedPolicyChoice[] {
	if (firstAvailable && secondAvailable)
		return ["none", "first", "second", "both"];
	if (firstAvailable) return ["none", "first"];
	if (secondAvailable) return ["none", "second"];
	return [];
}
