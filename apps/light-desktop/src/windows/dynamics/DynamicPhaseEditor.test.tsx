// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { AppProvider } from "../../state/AppContext";
import css from "../DynamicsWindow.css?raw";
import { createDefaultDynamicDefinition, DynamicEditor } from "./DynamicsEditor";

let style: HTMLStyleElement;
beforeEach(() => {
	const entries = new Map<string, string>();
	vi.stubGlobal("localStorage", { getItem: (key: string) => entries.get(key) ?? null,
		setItem: (key: string, value: string) => { entries.set(key, value); },
		removeItem: (key: string) => { entries.delete(key); }, clear: () => entries.clear() });
	style = document.createElement("style");
	style.textContent = css;
	document.head.appendChild(style);
});
afterEach(() => { cleanup(); style.remove(); vi.unstubAllGlobals(); });

it("mounts one visible authoritative Phase ordering with all distribution controls in the full editor", () => {
	const body = createDefaultDynamicDefinition(149, "intensity");
	body.target_binding = { type: "live_group", group_id: "25" };
	body.phase.ordering = { type: "grid_linear", angle_degrees: 45 };
	const mutate = vi.fn(async () => undefined);
	render(<AppProvider><DynamicEditor dynamic={{ kind: "dynamic" as const, id: body.id, revision: 7, updated_at: "", body }} view="phase"
		compact={false} busy={false} error={null} attributes={[]} presets={[]} runtime={null}
		selection={[]} selectedGroupId={null} onBack={vi.fn()} onMutate={mutate} onDelete={vi.fn()}
		onMove={vi.fn()} onCopy={vi.fn()} /></AppProvider>);
	expect(screen.getAllByRole("radiogroup", { name: "Ordering mode", hidden: true })).toHaveLength(1);
	const order = screen.getByRole("radiogroup", { name: "Ordering mode" });
	expect(getComputedStyle(order.closest(".dynamic-phase-ordering-field")!).display).not.toBe("none");
	for (const label of ["Direction", "Offset", "Span", "Blocks", "Repeats", "Explicit anchors"])
		expect(screen.getByLabelText(label)).toBeVisible();
	expect(screen.getByRole("radio", { name: /^Grid$/ })).toHaveAttribute("aria-checked", "true");
	expect(mutate).not.toHaveBeenCalled();
});
