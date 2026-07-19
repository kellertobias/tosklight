import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import {
	type BenchContractContext,
	type BenchUiContext,
	expect,
} from "../../apps/control-ui/e2e/bench/fixtures";
import { object } from "../support/catalog";

export async function setSequenceMasterFade(api: ApiDriver, millis: number) {
	const configuration = await api.request<any>("GET", "/api/v1/configuration");
	await api.request("PUT", "/api/v1/configuration", {
		...configuration,
		programmer_fade_millis: millis,
		sequence_master_fade_millis: millis,
	});
}

export async function currentProgrammer(api: ApiDriver): Promise<any> {
	const programmers = await api.request<any[]>("GET", "/api/v1/programmers");
	return programmers.find(
		(programmer) => programmer.session_id === api.session!.session_id,
	);
}

export async function cueListIdForPlayback(
	api: ApiDriver,
	playbackNumber: number,
): Promise<string> {
	const playback = await object<any>(api, "playback", String(playbackNumber));
	expect(playback.body.target.type).toBe("cue_list");
	return playback.body.target.cue_list_id;
}

export async function visualizationLevel(
	api: ApiDriver,
	fixtureId: string,
	attribute: string,
): Promise<number> {
	const visualization = await api.request<any>("GET", "/api/v1/visualization");
	const value = visualization.values.find(
		(item: any) =>
			item.fixture_id === fixtureId && item.attribute === attribute,
	)?.value;
	return rounded(typeof value === "number" ? value : (value?.value ?? 0));
}

export async function visualizationAfterTick(
	api: ApiDriver,
	bench: BenchContractContext["bench"],
	fixtureId: string,
	attribute: string,
	millis: number,
): Promise<number> {
	await bench.tick(millis);
	return visualizationLevel(api, fixtureId, attribute);
}

export async function rgbValues(
	api: ApiDriver,
	fixtureId: string,
): Promise<[number, number, number]> {
	return Promise.all(
		["red", "green", "blue"].map((attribute) =>
			visualizationLevel(api, fixtureId, attribute),
		),
	) as Promise<[number, number, number]>;
}

export function groupValues(cue: any): Record<string, number> {
	return Object.fromEntries(
		cue.group_changes.map((change: any) => [
			`${change.group_id}:${change.attribute}`,
			rounded(change.value?.value),
		]),
	);
}

export function rounded(value: number): number {
	return Math.round(value * 1_000_000) / 1_000_000;
}

export async function openEventStream(
	api: ApiDriver,
): Promise<{ socket: WebSocket; events: any[] }> {
	const socket = new WebSocket(
		api.baseUrl.replace(/^http/, "ws") + "/api/v1/events",
		["light.v1", `light.token.${api.session!.token}`],
	);
	await new Promise<void>((resolve, reject) => {
		const timeout = setTimeout(
			() => reject(new Error("event stream connection timed out")),
			5_000,
		);
		socket.addEventListener(
			"open",
			() => {
				clearTimeout(timeout);
				resolve();
			},
			{ once: true },
		);
		socket.addEventListener(
			"error",
			() => {
				clearTimeout(timeout);
				reject(new Error("event stream connection failed"));
			},
			{ once: true },
		);
	});
	const events: any[] = [];
	socket.addEventListener("message", (message) => {
		const event = JSON.parse(String(message.data));
		if (event.kind) events.push(event);
	});
	return { socket, events };
}

export async function showObjectEventAfter(
	events: any[],
	mark: number,
	id: string,
): Promise<any> {
	await expect
		.poll(
			() =>
				events
					.slice(mark)
					.find(
						(event) =>
							event.kind === "show_object_changed" &&
							event.payload?.kind === "cue_list" &&
							event.payload?.id === id,
					) ?? null,
		)
		.not.toBeNull();
	return events
		.slice(mark)
		.find(
			(event) =>
				event.kind === "show_object_changed" &&
				event.payload?.kind === "cue_list" &&
				event.payload?.id === id,
		);
}

export function eventIdentity(event: any) {
	return {
		kind: event.kind,
		objectKind: event.payload.kind,
		id: event.payload.id,
	};
}

export async function setCueOnlyFromUi(
	page: BenchUiContext["page"],
	checked: boolean,
) {
	const record = page.getByRole("button", {
		name: /REC(?: ARMED)?/,
		exact: true,
	});
	await record.hover();
	await page.mouse.down();
	await page.waitForTimeout(700);
	await page.mouse.up();
	const dialog = page.locator(".store-settings-modal");
	await expect(dialog).toBeVisible();
	const cueOnly = dialog.getByLabel("Cue only");
	if ((await cueOnly.isChecked()) !== checked) {
		await dialog.locator("label").filter({ hasText: "Cue only" }).click();
	}
	if (checked) await expect(cueOnly).toBeChecked();
	else await expect(cueOnly).not.toBeChecked();
	await dialog.getByRole("button", { name: "Done", exact: true }).click();
	await expect(dialog).toBeHidden();
	await page.waitForTimeout(1_000);
}
