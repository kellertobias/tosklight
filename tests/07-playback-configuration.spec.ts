import { test } from "../apps/control-ui/e2e/bench/fixtures";
import {
	registerPbk001PairedScenario,
	registerPbk001PhysicalControlsScenario,
	registerPbk001ReadApiScenario,
	registerPbk001VirtualCellsScenario,
} from "./playbackConfiguration/pbk001";
import {
	registerPbk002AtomicConfigurationScenario,
	registerPbk002LayoutUiScenario,
	registerPbk002PairedScenario,
} from "./playbackConfiguration/pbk002";
import {
	registerPbk003ActionMatrixScenario,
	registerPbk003PairedScenario,
	registerPbk003PhysicalFeedbackScenario,
} from "./playbackConfiguration/pbk003";
import {
	registerPbk004OwnershipScenario,
	registerPbk004PairedScenario,
	registerPbk004ReloadFeedbackScenario,
} from "./playbackConfiguration/pbk004";
import {
	registerPbk005FeedbackScenario,
	registerPbk005LifecycleScenario,
	registerPbk005PairedScenario,
} from "./playbackConfiguration/pbk005";
import {
	registerPbk006ActionMatrixScenario,
	registerPbk006OscScenario,
	registerPbk006PairedScenario,
	registerPbk006UiScenario,
} from "./playbackConfiguration/pbk006";

const PLAYBACK_CONFIGURATION_SCENARIOS =
	"docs/testing/07-playback-configuration.md";

test.describe(PLAYBACK_CONFIGURATION_SCENARIOS, () => {
	registerPbk001PairedScenario();
	registerPbk001ReadApiScenario();
	registerPbk001PhysicalControlsScenario();
	registerPbk001VirtualCellsScenario();
	registerPbk002PairedScenario();
	registerPbk002AtomicConfigurationScenario();
	registerPbk002LayoutUiScenario();
	registerPbk003PairedScenario();
	registerPbk003ActionMatrixScenario();
	registerPbk003PhysicalFeedbackScenario();
	registerPbk004PairedScenario();
	registerPbk004OwnershipScenario();
	registerPbk004ReloadFeedbackScenario();
	registerPbk005PairedScenario();
	registerPbk005LifecycleScenario();
	registerPbk005FeedbackScenario();
	registerPbk006PairedScenario();
	registerPbk006ActionMatrixScenario();
	registerPbk006UiScenario();
	registerPbk006OscScenario();
});
