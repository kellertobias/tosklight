import { describe, expect, it } from "vitest";
import { windowChoices } from "../components/modals/WindowPicker";
import { builtIns } from "../components/shell/LeftDock";
import { windowRegistry } from "./WindowRegistry";

describe("Running pane registration", () => {
	it("is available through Open Window but absent from Built-ins", () => {
		expect(windowChoices).toContainEqual(["running", "Running"]);
		expect(windowRegistry.running).toBeDefined();
		expect(builtIns.map(([kind]) => kind)).not.toContain("running");
	});
});
