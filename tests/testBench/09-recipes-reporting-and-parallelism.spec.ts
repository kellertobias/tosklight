import { test, expect } from "../bench/core/fixtures";
import { ApiDriver } from "../bench/core/api";
import { LightBench } from "../bench/core/lightBench";
import { scenario } from "../bench/core/scenario";
import { fixtureRange } from "../bench/command-selection/selectionContract";
import { Show } from "../bench/show/showCatalog";

scenario(
	"BENCH-RECIPE-001",
	"recipes expose expanded semantic actions and reject unsupported routes before mutation",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.recipe.run("select-representative-fixtures", {
			selection: [fixtureRange(1, 4)],
		});
		await t.expect.selection(fixtureRange(1, 4));

		const identity = t.show.contractIdentity();
		await expect(
			t.recipe.run("use-canonical-show", { show: Show.Empty }, "osc"),
		).rejects.toThrow(/does not support route "osc"/);
		expect(t.show.contractIdentity()).toEqual(identity);

		await t.recipe.run("use-canonical-show", {
			show: Show.TwelveDimmers,
		});
		const report = t.recipe.reports.at(-1);
		expect(report).toMatchObject({
			name: "use-canonical-show",
			route: "ui",
		});
		expect(report?.steps.map((step) => step.title)).toContain("FIXTURE SETUP");
	},
);

test("BENCH-PARALLEL-001 @bench @api › five benches isolate ports, shows, sessions, clocks, and process cleanup", async () => {
	test.setTimeout(90_000);
	const benches = Array.from({ length: 5 }, () => new LightBench());
	try {
		await Promise.all(benches.map((bench, index) => bench.start(100 + index)));
		const shows = await Promise.all(
			benches.map((bench, index) => bench.createTwelveDimmerShow(`parallel-${index}`)),
		);
		expect(new Set(benches.map((bench) => bench.dataDir)).size).toBe(5);
		expect(new Set(benches.map((bench) => bench.baseUrl)).size).toBe(5);
		expect(new Set(benches.map((bench) => bench.oscPort)).size).toBe(5);
		expect(new Set(benches.map((bench) => bench.artnet.port)).size).toBe(5);
		expect(new Set(benches.map((bench) => bench.sacn.port)).size).toBe(5);
		expect(new Set(shows.map((show) => show.id)).size).toBe(5);
		expect(new Set(shows.map((show) => show.session.session_id)).size).toBe(5);

		await Promise.all(
			benches.map(async (bench, index) => {
				const api = new ApiDriver(bench.baseUrl);
				api.session = shows[index].session;
				await api.setCommandLineText(`FIXTURE ${index + 1}`);
				expect((await api.getCommandLine()).commandLine.text).toBe(
					`FIXTURE ${index + 1}`,
				);
			}),
		);
		const [freeRun, deterministic] = await Promise.all([
			benches[0].freeRunClock(80),
			benches[1].tick(250),
		]);
		expect(freeRun.wall_millis).toBeGreaterThanOrEqual(80);
		expect(deterministic.revision).toBeGreaterThan(0);

		await benches[0].stopServerAbruptly();
		const survivor = new ApiDriver(benches[1].baseUrl);
		survivor.session = shows[1].session;
		expect((await survivor.getCommandLine()).commandLine.text).toBe("FIXTURE 2");
		expect(await survivor.shows<{ id: string }>()).toContainEqual(
			expect.objectContaining({ id: shows[1].id }),
		);
	} finally {
		await Promise.allSettled(benches.map((bench) => bench.stop()));
	}
});
