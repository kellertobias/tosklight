import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useServerFeatureStores } from "./useServerFeatureStores";

describe("useServerFeatureStores Output runtime", () => {
	it("retains exactly one Output store across server-state renders", () => {
		const rendered = renderHook(() => useServerFeatureStores());
		const store = rendered.result.current.outputRuntimeStore;

		rendered.rerender();
		expect(rendered.result.current.outputRuntimeStore).toBe(store);
	});
});
