import { test } from "../apps/control-ui/e2e/bench/fixtures";
import { registerCaptureMaskScenarios } from "./preloadVirtualPlaybackContracts/captureMaskScenarios";
import { registerCombinedPreloadScenarios } from "./preloadVirtualPlaybackContracts/combinedPreloadScenarios";
import { registerLayoutPersistenceScenarios } from "./preloadVirtualPlaybackContracts/layoutPersistenceScenarios";
import { registerPhysicalPlaybackPreloadScenarios } from "./preloadVirtualPlaybackContracts/physicalPlaybackPreloadScenarios";
import { registerProgrammerPreloadScenarios } from "./preloadVirtualPlaybackContracts/programmerPreloadScenarios";
import { registerVirtualPlaybackPreloadScenarios } from "./preloadVirtualPlaybackContracts/virtualPlaybackPreloadScenarios";
import { registerVirtualZoneApiScenario } from "./preloadVirtualPlaybackContracts/virtualZoneApiScenario";
import { registerVirtualZonePairScenario } from "./preloadVirtualPlaybackContracts/virtualZonePairScenario";
import { registerVirtualZoneUiScenario } from "./preloadVirtualPlaybackContracts/virtualZoneUiScenario";

test.describe("docs/testing/06-preload-modes-and-virtual-playbacks.md", () => {
	test.describe.configure({ mode: "serial" });

	registerProgrammerPreloadScenarios();
	registerPhysicalPlaybackPreloadScenarios();
	registerLayoutPersistenceScenarios();
	registerVirtualPlaybackPreloadScenarios();
	registerCaptureMaskScenarios();
	registerCombinedPreloadScenarios();
	registerVirtualZonePairScenario();
	registerVirtualZoneApiScenario();
	registerVirtualZoneUiScenario();
});
