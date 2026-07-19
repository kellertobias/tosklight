import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { assignMatterRestartPlayback } from "./05-virtual-time-persistence-and-recovery.playback-helpers";
import {
	assertFadeBoundaries,
	assertZeroTicks,
	connectHardware,
	disconnectHardware,
	expectEncoderTarget,
	FIXED_NOW,
	fixtureRow,
	type HardwareState,
	openFixtures,
	setDimmerByTouch,
	setProgrammerFade,
	setProgrammerFadeThroughUi,
} from "./05-virtual-time-persistence-and-recovery.time-helpers";
import { fixtureIdsByNumber, loadCanonicalCopy } from "./support/catalog";

export function registerMatterRestartTest(): void {
	test("MATTER-002 @restart › desk enablement survives show changes and restart while advertised playbacks follow the active show", async ({
		api,
		bench,
	}) => {
		const showA = await loadCanonicalCopy(api, bench, "matter-002-a");
		const configuration = await api.request<any>(
			"GET",
			"/api/v1/configuration",
			undefined,
			false,
		);
		await api.request("PUT", "/api/v1/configuration", {
			...configuration.configuration,
			matter_enabled: true,
		});
		const assignment = await assignMatterRestartPlayback(api);
		const endpointId = 1 + (assignment.page - 1) * 127 + (assignment.slot - 1);
		await expect
			.poll(async () => {
				const status = await api.request<any>("GET", "/api/v1/matter/status");
				return status.lights.find(
					(light: any) => light.endpoint_id === endpointId,
				)?.playback_number;
			})
			.toBe(assignment.playbackNumber);

		const showB = await loadCanonicalCopy(api, bench, "matter-002-b");
		expect(showB.id).not.toBe(showA.id);
		await expect
			.poll(async () => {
				const status = await api.request<any>("GET", "/api/v1/matter/status");
				return status.lights.some(
					(light: any) => light.endpoint_id === endpointId,
				);
			})
			.toBe(false);
		expect(
			(await api.request<any>("GET", "/api/v1/configuration", undefined, false))
				.configuration.matter_enabled,
		).toBe(true);

		await api.request("POST", `/api/v1/shows/${showA.id}/open`, {
			transition: "hold_current",
		});
		await expect
			.poll(async () => {
				const status = await api.request<any>("GET", "/api/v1/matter/status");
				return status.lights.find(
					(light: any) => light.endpoint_id === endpointId,
				)?.playback_number;
			})
			.toBe(assignment.playbackNumber);

		await bench.stopServerGracefully(api.session!.token);
		await bench.startServer();
		await api.login();
		expect(
			(await api.request<any>("GET", "/api/v1/configuration", undefined, false))
				.configuration.matter_enabled,
		).toBe(true);
		expect(
			(await api.request<any>("GET", "/api/v1/bootstrap", undefined, false))
				.active_show.id,
		).toBe(showA.id);
		await expect
			.poll(async () => {
				const status = await api.request<any>("GET", "/api/v1/matter/status");
				return status.lights.find(
					(light: any) => light.endpoint_id === endpointId,
				)?.playback_number;
			})
			.toBe(assignment.playbackNumber);

		const enabled = await api.request<any>(
			"GET",
			"/api/v1/configuration",
			undefined,
			false,
		);
		await api.request("PUT", "/api/v1/configuration", {
			...enabled.configuration,
			matter_enabled: false,
		});
		await bench.stopServerGracefully(api.session!.token);
		await bench.startServer();
		await api.login();
		expect(
			(await api.request<any>("GET", "/api/v1/configuration", undefined, false))
				.configuration.matter_enabled,
		).toBe(false);
		expect(
			(await api.request<any>("GET", "/api/v1/matter/status")).lights,
		).toEqual([]);
	});
}

export function registerZeroTickScenario(): void {
	pairedScenario<HardwareState>({
		id: "TIME-001",
		title: "zero ticks emit current state without advancing behavior time",
		arrange: async ({ api, bench }, surface) => {
			await loadCanonicalCopy(api, bench, `time-001-${surface}`);
			expect(
				(
					await api.request<{ now: string }>(
						"POST",
						"/api/v1/test/clock/reset",
						undefined,
						false,
					)
				).now,
			).toBe(FIXED_NOW);
			// Reset deliberately clears the test-bench programmer registry. Reconnect the durable user
			// so this surface starts with the same production session/programmer relationship as the UI.
			await api.login();
			return {};
		},
		api: async ({ api }) => {
			await setProgrammerFade(api, 0, 3_000);
			await api.executeCommandLine("FIXTURE 1 AT 50");
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await connectHardware(api, bench, state, "time-001-ui");
			await desk.open(bench.baseUrl);
			await setProgrammerFadeThroughUi(api, page, 0);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 50);
		},
		assert: async ({ api, bench }, state) => {
			try {
				await assertZeroTicks(api, bench);
			} finally {
				await disconnectHardware(api, state);
			}
		},
	});
}

export function registerFadeBoundaryScenario(): void {
	pairedScenario<{ fixtureId: string } & HardwareState>({
		id: "TIME-002",
		title: "all programmer-fade boundaries are exact",
		arrange: async ({ api, bench }, surface) => {
			await loadCanonicalCopy(api, bench, `time-002-${surface}`);
			return { fixtureId: (await fixtureIdsByNumber(api))[1] };
		},
		api: async ({ api, bench }) => {
			await setProgrammerFade(api, 3_000);
			await api.executeCommandLine("FIXTURE 1 AT 0");
			await bench.tick(3_000);
			await api.executeCommandLine("FIXTURE 1 AT 100");
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await connectHardware(api, bench, state, "time-002-ui");
			await desk.open(bench.baseUrl);
			await setProgrammerFadeThroughUi(api, page, 3);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 0);
			await bench.tick(3_000);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 100);
			await expectEncoderTarget(page, 100);
		},
		assert: async ({ api, bench }, state) => {
			try {
				await assertFadeBoundaries(api, bench, state.fixtureId);
			} finally {
				await disconnectHardware(api, state);
			}
		},
	});
}
