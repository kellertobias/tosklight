import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useServerFeatureStores } from "./useServerFeatureStores";

describe("useServerFeatureStores Speed Group runtime", () => {
	it("retains exactly one Speed Group store across broad state renders", () => {
		const rendered = renderHook(() => useServerFeatureStores());
		const store = rendered.result.current.speedGroupRuntimeStore;

		rendered.rerender();
		expect(rendered.result.current.speedGroupRuntimeStore).toBe(store);
	});
});
