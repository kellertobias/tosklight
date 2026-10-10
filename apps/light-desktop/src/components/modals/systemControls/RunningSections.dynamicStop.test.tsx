import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RunningSections } from "./RunningSections";
import type { RunningDynamicController } from "./runningDynamicsAuthority";
afterEach(cleanup);
const base: RunningDynamicController = {
	key: "instance:controller",
	instanceId: "instance",
	dynamicId: "dynamic",
	poolNumber: 100,
	name: "Sinus",
	targets: ["fixture"],
	pending: false,
	instancePaused: false,
	speedSource: "E",
	controllerId: "controller",
	source: "Cue 1",
	priority: 0,
	size: 1,
	speedMultiplier: 1,
	phaseOffsetDegrees: 0,
	paused: false,
	winning: true,
	releasing: false,
	activationMix: 1,
};
function view(row: RunningDynamicController) {
	const stop = vi.fn();
	render(
		<RunningSections
			playbacks={[]}
			dynamics={[row]}
			dynamicsLoading={false}
			dynamicsError={null}
			dynamicsCanStop
			stoppingDynamicControllerIds={new Set()}
			preloadActive={false}
			playbacksLoading={false}
			releaseAvailable
			onReleasePlayback={vi.fn()}
			onReleasePreload={vi.fn()}
			onTurnOffDynamic={stop}
		/>,
	);
	return stop;
}
describe("Running Dynamic source-specific Stop affordance", () => {
	it("disables unknown or subordinate sources with local guidance", () => {
		const stop = view({
			...base,
			stopGuidance:
				"Stop this source from its owning control; no exact stop owner is available.",
		});
		const button = screen.getByRole("button", { name: /Turn off Dynamic 100/ });
		expect(button).toBeDisabled();
		fireEvent.click(button);
		expect(stop).not.toHaveBeenCalled();
		expect(screen.getByText(/no exact stop owner/)).toBeVisible();
	});
	it("keeps trusted Programmer authoring wording distinct from live Stop", () => {
		view({
			...base,
			source: "Programmer",
			stopMode: "programmer",
			stopGuidance:
				"Edits the current Programmer; follows its Preload capture mode.",
		});
		expect(
			screen.getByRole("button", { name: /Turn off Dynamic/ }),
		).toHaveTextContent("Off in Programmer");
		expect(screen.getByText(/follows its Preload capture mode/)).toBeVisible();
	});
	it("enables the exact typed virtual row without parsing its display label", () => {
		const row = {
			...base,
			source: "Renamed label",
			stopMode: "playback" as const,
			stopOwner: {
				kind: "virtual_playback" as const,
				page: 2,
				playback_number: 1303,
			},
		};
		const stop = view(row);
		const button = screen.getByRole("button", { name: /Turn off Dynamic/ });
		expect(button).toBeEnabled();
		fireEvent.click(button);
		expect(stop).toHaveBeenCalledExactlyOnceWith(row);
	});
});
