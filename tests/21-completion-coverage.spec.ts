// @bench-semantic-world

import { scenario } from "./bench/core/scenario";

scenario(
	"HIGHLIGHT-006",
	"the production hardware simulator preserves geometry and sends independent full-height faders",
	async (t) => {
		await t.hardwareSimulator.expectGeometryAndIndependentFaders();
	},
);

scenario(
	"ENCODER-DISPLAY-001",
	"simulator touch controls emit encoder and NAV turn, held-turn, and click tokens",
	async (t) => {
		await t.hardwareSimulator.expectEncoderAndNavigationTokens();
	},
);

scenario(
	"UPDATE-002",
	"actual simulator pointer gestures emit complete, mutually exclusive Shift and Record sequences",
	async (t) => {
		await t.hardwareSimulator.expectShiftRecordPointerSequences();
	},
);
