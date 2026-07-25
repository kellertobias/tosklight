import { test as base, type PlaywrightTestArgs } from "@playwright/test";
import { ApiDriver } from "./api";
import { DeskDriver } from "./desk";
import { LightBench, type TestShow } from "./lightBench";

export interface TestFixtures {
	api: ApiDriver;
	bench: LightBench;
	desk: DeskDriver;
	show: TestShow;
}
export type BenchContractContext = Pick<
	TestFixtures,
	"api" | "bench" | "show"
> &
	Pick<PlaywrightTestArgs, "request">;
export type BenchUiContext = BenchContractContext &
	Pick<TestFixtures, "desk"> &
	Pick<PlaywrightTestArgs, "page">;

const extendedTest = base.extend<TestFixtures>({
	bench: async ({}, use, testInfo) => {
		const bench = new LightBench();
		await bench.start(testInfo.workerIndex);
		try {
			await use(bench);
		} finally {
			await bench.stop();
		}
	},
	baseURL: async ({ bench }, use) => use(bench.baseUrl),
	show: [
		async ({ bench }, use, testInfo) => {
			const show = await bench.createTwelveDimmerShow();
			await use(show);
			if (testInfo.status !== testInfo.expectedStatus) {
				for (const [name, body] of Object.entries(
					await bench.failureArtifacts(show.session.token),
				)) {
					await testInfo.attach(name, {
						body: Buffer.from(body),
						contentType: name.endsWith(".json")
							? "application/json"
							: "text/plain",
					});
				}
			}
		},
		{ auto: true },
	],
	api: async ({ bench, show }, use) => {
		const api = new ApiDriver(bench.baseUrl);
		api.session = show.session;
		await use(api);
	},
	desk: async ({ page, api, bench }, use, testInfo) => {
		const driver = new DeskDriver(
			page,
			testInfo.title,
			api.session?.desk.id ?? null,
			() => bench.visualOscSummary(),
		);
		try {
			await use(driver);
		} finally {
			await driver.dispose();
		}
	},
});

const primarySurface = /@(api|ui|desktop)\b/u;
const retiredSurface = /@(supplemental(?:-api|-ui)?|osc|restart|wire)\b/u;

const registerClassifiedTest: NonNullable<
	ProxyHandler<typeof extendedTest>["apply"]
> = (target, thisArgument, argumentsList) => {
	const title = argumentsList[0];
	if (typeof title === "string" && !primarySurface.test(title)) {
		if (retiredSurface.test(title)) return undefined;
		throw new Error(
			`Playwright test "${title}" has no primary @api, @ui, or @desktop surface`,
		);
	}
	return Reflect.apply(target, thisArgument, argumentsList);
};

/**
 * Only primary suites are registered. Legacy supplemental/OSC/restart/wire
 * variants were replaced by semantic UI scenarios or reclassified as explicit
 * API failure-mode contracts. Keeping the registration boundary strict avoids
 * silently rebuilding a second catch-all suite.
 */
export const test = new Proxy(extendedTest, {
	apply: registerClassifiedTest,
	get(target, property, receiver) {
		const value = Reflect.get(target, property, receiver);
		if (
			typeof value === "function" &&
			(property === "only" || property === "skip" || property === "fixme")
		) {
			return new Proxy(value, { apply: registerClassifiedTest });
		}
		return value;
	},
});

export { expect } from "@playwright/test";
