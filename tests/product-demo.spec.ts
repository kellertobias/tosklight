// @bench-semantic-world

import { scenario } from "./bench/core/scenario";

scenario(
	"BENCH-PRODUCT-DEMO-001",
	"narrates the complete Full HD product demo surface in one regression run",
	async (t) => {
		await t.demo.run();
	},
);
