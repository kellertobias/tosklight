import { test } from "../apps/control-ui/e2e/bench/fixtures";
import {
	registerCompatibleProfileMigrationTest,
	registerFreshProfileStartupTest,
	registerProfileRecoveryTests,
} from "./05-virtual-time-persistence-and-recovery.fixture-profile-tests";
import {
	registerFadeBoundaryScenario,
	registerMatterRestartTest,
	registerZeroTickScenario,
} from "./05-virtual-time-persistence-and-recovery.matter-and-fade-tests";
import {
	registerFixtureTimingTest,
	registerGroupTimingTest,
} from "./05-virtual-time-persistence-and-recovery.programmer-timing-tests";
import { registerRevisionCopyScenario } from "./05-virtual-time-persistence-and-recovery.revision-copy-tests";
import {
	registerAtomicRecoveryTests,
	registerCorruptActiveShowRecoveryTests,
	registerLegacyMigrationTests,
	registerMalformedRecoveryScenario,
} from "./05-virtual-time-persistence-and-recovery.show-recovery-tests";
import {
	registerEmptyShowRestartTest,
	registerShow001PairedScenario,
	registerShow001ProcessRestartTest,
} from "./05-virtual-time-persistence-and-recovery.show-restart-tests";
import { registerVirtualBehaviorTest } from "./05-virtual-time-persistence-and-recovery.virtual-behavior-tests";

const SCENARIO = "docs/testing/05-virtual-time-persistence-and-recovery.md";

test.describe(SCENARIO, () => {
	test.setTimeout(90_000);

	registerMatterRestartTest();
	registerZeroTickScenario();
	registerFadeBoundaryScenario();
	registerFixtureTimingTest();
	registerGroupTimingTest();
	registerVirtualBehaviorTest();
	registerShow001PairedScenario();
	registerEmptyShowRestartTest();
	registerShow001ProcessRestartTest();
	registerAtomicRecoveryTests();
	registerMalformedRecoveryScenario();
	registerCorruptActiveShowRecoveryTests();
	registerLegacyMigrationTests();
	registerFreshProfileStartupTest();
	registerCompatibleProfileMigrationTest();
	registerProfileRecoveryTests();
	registerRevisionCopyScenario();
});
