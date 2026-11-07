import type { SpeedGroupSnapshot } from "../apps/light-desktop/src/api/generated/light-wire";
import { expect, test } from "./bench/core/fixtures";

test("OSC Speed Groups A–E accept absolute integer and fractional BPM @api", async ({
	api,
	bench,
}) => {
	const clientId = "speed-group-bpm";
	const hardware = await bench.osc();
	const readTempos = async () => {
		const snapshot = await api.request<SpeedGroupSnapshot>(
			"GET",
			"/api/v2/speed-groups",
		);
		return Object.fromEntries(
			snapshot.projection.groups.map(({ group, manual_bpm }) => [
				group,
				manual_bpm,
			]),
		);
	};

	await hardware.subscribe(clientId, "desk");
	try {
		const groups = ["A", "B", "C", "D", "E"] as const;
		const wholeTempos = [93, 108, 121, 137, 152] as const;
		for (const fraction of [0, 0.5]) {
			for (const [index, bpm] of wholeTempos.entries()) {
				await hardware.send(`/light/desk/speed-group/${index + 1}/encoder`, [
					bpm + fraction,
				]);
			}
			await expect.poll(readTempos).toEqual(
				Object.fromEntries(
					groups.map((group, index) => [group, wholeTempos[index] + fraction]),
				),
			);
		}

		await hardware.send("/light/desk/speed-group/1/encoder", ["170"]);
		await hardware.send("/light/desk/speed-group/6/encoder", [180]);
		// A valid message from the same socket confirms ingress processed the earlier packets.
		await hardware.send("/light/desk/speed-group/5/encoder", [160.75]);
		await expect.poll(readTempos).toEqual({
			A: 93.5,
			B: 108.5,
			C: 121.5,
			D: 137.5,
			E: 160.75,
		});
	} finally {
		await hardware.unsubscribe(clientId);
	}
});
