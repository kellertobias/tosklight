import { act, cleanup, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { NativeColorPagesSnapshot } from "../../api/nativeColorModels";
import { nativePages } from "../../components/control/parameterControls/familyEncoders/nativeColorTestSupport";
import { DisplayedSourceReadouts } from "../programmerValues/displayedSource";
import { FamilyEncodersContextProvider, type FamilyEncodersContextValue } from "./FamilyEncodersProvider";
import {
	NATIVE_VALUES_RETRIES,
	NATIVE_VALUES_RETRY_MILLIS,
	useNativeColorPages,
} from "./useNativeColorPages";

const FIXTURES = ["11111111-1111-4111-8111-111111111111"];

function mount(answers: NativeColorPagesSnapshot[]) {
	let reads = 0;
	const context: FamilyEncodersContextValue = {
		loadPages: async () => {
			throw new Error("unused");
		},
		loadNativePages: async () => answers[Math.min(reads++, answers.length - 1)],
		readouts: new DisplayedSourceReadouts({
			request: async () => ({ lane: "normal", scope: {}, revision: 1, owners: [] }),
		}),
		session: null,
	};
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={context}>{children}</FamilyEncodersContextProvider>
	);
	const hook = renderHook(() => useNativeColorPages(FIXTURES, true), { wrapper });
	return { hook, reads: () => reads };
}

async function settle(millis = 0) {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(millis);
	});
}

describe("Direct Color pages (TL-653)", () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => {
		cleanup();
		vi.useRealTimers();
	});

	it("reads a reference head's values again until an accepted frame provides them", async () => {
		const { hook, reads } = mount([nativePages(4, false), nativePages(4, false), nativePages(4, true)]);
		await settle();
		expect(reads()).toBe(1);
		expect(hook.result.current?.values ?? null).toBeNull();
		await settle(NATIVE_VALUES_RETRY_MILLIS);
		expect(reads()).toBe(2);
		await settle(NATIVE_VALUES_RETRY_MILLIS);
		expect(reads()).toBe(3);
		expect(hook.result.current?.values).not.toBeNull();
		// Values present: no further reads.
		await settle(NATIVE_VALUES_RETRY_MILLIS * 4);
		expect(reads()).toBe(3);
	});

	it("gives up after a bounded number of re-reads", async () => {
		const { reads } = mount([nativePages(4, false)]);
		await settle(NATIVE_VALUES_RETRY_MILLIS * (NATIVE_VALUES_RETRIES + 5));
		expect(reads()).toBe(NATIVE_VALUES_RETRIES + 1);
	});
});
