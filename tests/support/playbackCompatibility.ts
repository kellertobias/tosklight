import { expect } from "../bench/core/fixtures";
import type { ApiDriver } from "../bench/core/api";
import type { LightBench } from "../bench/core/lightBench";
import {
	fileHash,
	showEntry,
} from "../05-virtual-time-persistence-and-recovery.show-helpers";

/** Offline corruption belongs only to the isolated acceptance server's owned Show. */
export async function assertLegacyPlaybackRefusalAndRecovery(
	api: ApiDriver,
	bench: LightBench,
	showId: string,
	stageRetiredShape: (file: string) => Promise<void>,
) {
	const entry = await showEntry(api, showId);
	const validBytes = await api.downloadShow(showId);
	await bench.stopServerGracefully(api.session!.token);
	await stageRetiredShape(entry.path);
	const originalHash = await fileHash(entry.path);
	for (let run = 0; run < 2; run++) {
		await bench.startServer();
		await api.login();
		const readiness = await api.request<any>(
			"GET",
			"/api/v2/readiness",
			undefined,
			false,
		);
		expect(readiness).toMatchObject({ status: "ready", recovery_mode: true });
		expect(readiness.active_show_error).toMatch(
			/Playback|playback|legacy|incompatible/,
		);
		const frame = await bench.tick(0);
		expect(
			frame.universes.every((universe) =>
				universe.slots.every((value) => value === 0),
			),
		).toBe(true);
		expect(await fileHash(entry.path)).toBe(originalHash);
		if (run === 0) await bench.stopServerGracefully(api.session!.token);
	}
	const recovered = await api.createShow<{ id: string }>({
		name: `canonical-recovery-${crypto.randomUUID()}`,
		data_base64: validBytes.toString("base64"),
		overwrite: false,
	});
	await api.openShow(recovered.id, { transition: "safe_blackout" });
	const bootstrap = await api.request<any>(
		"GET",
		"/api/v2/bootstrap",
		undefined,
		false,
	);
	expect(bootstrap.active_show.id).toBe(recovered.id);
	expect(bootstrap.active_show_error).toBeNull();
	expect(await fileHash(entry.path)).toBe(originalHash);
	return recovered;
}
