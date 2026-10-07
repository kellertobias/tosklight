import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SelectiveImportOutcome } from "../../api/selectiveImportModels";
import {
	type SelectiveImportCapability,
	SelectiveImportProvider,
	type SelectiveImportSource,
	useSelectiveImport,
} from "./SelectiveImportContext";

const unchanged: SelectiveImportOutcome = {
	requestId: "request",
	correlationId: "correlation",
	changed: false,
	showId: "target",
	showRevision: 9,
	eventSequence: null,
	objectChanges: [],
	outcomes: [],
	profileChanges: [],
	managedAssets: [],
};

function source(): SelectiveImportSource {
	return {
		catalog: vi.fn(),
		preview: vi.fn(),
		apply: vi.fn().mockResolvedValue(unchanged),
		refreshCompatibilityState: vi.fn().mockResolvedValue(undefined),
		reportError: vi.fn(),
	};
}

afterEach(cleanup);

describe("SelectiveImportProvider", () => {
	it("owns apply refresh and error behavior behind the narrow capability", async () => {
		const capabilitySource = source();
		let capability!: SelectiveImportCapability;
		function Probe() {
			capability = useSelectiveImport();
			return null;
		}
		render(
			<SelectiveImportProvider source={capabilitySource}>
				<Probe />
			</SelectiveImportProvider>,
		);
		await act(() =>
			capability.apply("target", "source", {
				requestId: "request",
				expectedSourceRevision: 4,
				expectedTargetRevision: 9,
				mode: "replace_by_position" as const,
				selectedObjects: [],
				conflictResolutions: [],
				profileConflictResolutions: [],
			}),
		);

		expect(capabilitySource.apply).toHaveBeenCalledOnce();
		expect(capabilitySource.refreshCompatibilityState).not.toHaveBeenCalled();
		expect(capabilitySource.reportError).toHaveBeenLastCalledWith(null);
	});

	it("refreshes changed outcomes and reports failures", async () => {
		const capabilitySource = source();
		const failure = new Error("revision changed");
		failure.stack = "Error: revision changed\n    at applyImport (import.ts:24:6)";
		vi.mocked(capabilitySource.apply)
			.mockResolvedValueOnce({ ...unchanged, changed: true })
			.mockRejectedValueOnce(failure);
		let capability!: SelectiveImportCapability;
		function Probe() {
			capability = useSelectiveImport();
			return null;
		}
		render(
			<SelectiveImportProvider source={capabilitySource}>
				<Probe />
			</SelectiveImportProvider>,
		);
		const request = {
			requestId: "request",
			expectedSourceRevision: 4,
			expectedTargetRevision: 9,
			mode: "replace_by_position" as const,
			selectedObjects: [],
			conflictResolutions: [],
			profileConflictResolutions: [],
		};
		await act(() => capability.apply("target", "source", request));
		expect(capabilitySource.refreshCompatibilityState).toHaveBeenCalledOnce();
		await expect(capability.apply("target", "source", request)).rejects.toThrow(
			"revision changed",
		);
		expect(capabilitySource.reportError).toHaveBeenLastCalledWith(
			`${failure.message}\n${failure.stack}`,
		);
	});
});
