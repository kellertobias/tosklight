import type { OscBridge } from "./oscBridge";

export const OSC_TEST_CONTROL = "__lightOscTestControl";

export type ControllableOscWindow = Window & {
	[OSC_TEST_CONTROL]?: OscBridge;
};

export function injectedOscBridge(
	runtime: ControllableOscWindow,
): OscBridge | null {
	const bridge = runtime[OSC_TEST_CONTROL];
	if (
		!bridge ||
		typeof bridge.connect !== "function" ||
		typeof bridge.send !== "function" ||
		typeof bridge.listenFeedback !== "function"
	)
		return null;
	return bridge;
}
