import {
	type ApiDriver,
	commandLineOwnership,
} from "../../../apps/control-ui/e2e/bench/api";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { clearProgrammerValues } from "../../../apps/control-ui/e2e/bench/programmerValues";
import { loadCanonicalCopy } from "../catalog";
import type { ProgrammerState, ShowEntry, VersionedObject } from "./contracts";

export async function loadCompactRig(
	api: ApiDriver,
	bench: any,
	name: string,
): Promise<string> {
	const show = await loadCanonicalCopy(api, bench, name);
	await api.command("selection.set", { fixtures: [] });
	await clearProgrammerValues(api, { surface: "api", showId: show.id });
	const group4 = (await objects(api, "group")).find(
		(group) => group.id === "4",
	);
	await putObject(
		api,
		"group",
		"4",
		{
			id: "4",
			name: "Center Spot",
			fixtures: [],
			derived_from: null,
			frozen_from: null,
			programming: {},
			master: 1,
			playback_fader: null,
		},
		group4?.revision ?? 0,
	);
	return show.id;
}

export async function command(api: ApiDriver, value: string): Promise<void> {
	const ownership = commandLineOwnership(value);
	if (ownership.via === "compatibility") {
		await api.executeCompatibilityProgrammerCommand({
			family: ownership.family,
			command: value,
		});
		return;
	}
	await api.executeCommandLine(value);
}

export async function commandError(
	api: ApiDriver,
	value: string,
): Promise<string> {
	try {
		await command(api, value);
	} catch (error) {
		return error instanceof Error ? error.message : String(error);
	}
	throw new Error(`Expected command to fail: ${value}`);
}

export async function programmer(api: ApiDriver): Promise<ProgrammerState> {
	const programmers = await api.request<ProgrammerState[]>(
		"GET",
		"/api/v1/programmers",
	);
	const current =
		programmers.find(
			(item: any) => item.session_id === api.session?.session_id,
		) ?? programmers[0];
	expect(current).toBeDefined();
	return current;
}

export async function expectProgrammer(
	api: ApiDriver,
	assertion: (programmer: ProgrammerState) => void | Promise<void>,
): Promise<void> {
	await expect
		.poll(
			async () => {
				const programmers = await api.request<ProgrammerState[]>(
					"GET",
					"/api/v1/programmers",
				);
				let lastError: unknown = null;
				for (const snapshot of programmers) {
					try {
						await assertion(snapshot);
						return true;
					} catch (error) {
						lastError = error;
					}
				}
				if (lastError) throw lastError;
				throw new Error("No programmer matched assertion");
			},
			{ timeout: 2_000 },
		)
		.toBe(true);
}

export async function select(
	api: ApiDriver,
	fixtures: string[],
): Promise<void> {
	await api.command("selection.set", { fixtures });
}

export async function gestureFixture(
	api: ApiDriver,
	fixtureId: string,
	remove = false,
): Promise<void> {
	await api.command("selection.gesture", {
		source: { type: "fixture", fixture_id: fixtureId },
		remove,
	});
}

export async function gestureGroup(
	api: ApiDriver,
	groupId: string,
	remove = false,
): Promise<void> {
	await api.command("selection.gesture", {
		source: { type: "live_group", group_id: groupId },
		remove,
	});
}

export async function objects<T = Record<string, any>>(
	api: ApiDriver,
	kind: string,
): Promise<Array<VersionedObject<T>>> {
	const bootstrap = await api.request<{ active_show: ShowEntry | null }>(
		"GET",
		"/api/v1/bootstrap",
		undefined,
		false,
	);
	expect(bootstrap.active_show).toBeTruthy();
	const result = await api.request<Array<VersionedObject<T>>>(
		"GET",
		`/api/v1/shows/${bootstrap.active_show!.id}/objects/${kind}`,
		undefined,
		false,
	);
	return result.sort((left, right) =>
		left.id.localeCompare(right.id, undefined, { numeric: true }),
	);
}

export async function object<T = Record<string, any>>(
	api: ApiDriver,
	kind: string,
	id: string,
): Promise<VersionedObject<T>> {
	const found = (await objects<T>(api, kind)).find((item) => item.id === id);
	expect(found).toBeDefined();
	return found!;
}

export async function putObject(
	api: ApiDriver,
	kind: string,
	id: string,
	body: unknown,
	revision = 0,
): Promise<void> {
	const bootstrap = await api.request<{ active_show: ShowEntry | null }>(
		"GET",
		"/api/v1/bootstrap",
		undefined,
		false,
	);
	expect(bootstrap.active_show).toBeTruthy();
	await api.request(
		"PUT",
		`/api/v1/shows/${bootstrap.active_show!.id}/objects/${kind}/${id}`,
		body,
		true,
		revision,
	);
}

export async function fixtureIdsByNumber(
	api: ApiDriver,
): Promise<Record<number, string>> {
	const fixtures = await objects(api, "patched_fixture");
	return Object.fromEntries(
		fixtures.map((fixture) => [
			fixture.body.fixture_number,
			fixture.body.fixture_id,
		]),
	);
}

export async function fixtureNumberById(
	api: ApiDriver,
): Promise<Record<string, number>> {
	const fixtures = await objects(api, "patched_fixture");
	return Object.fromEntries(
		fixtures.map((fixture) => [
			fixture.body.fixture_id,
			fixture.body.fixture_number,
		]),
	);
}

export async function expectSelectedNumbers(
	api: ApiDriver,
	expected: number[],
): Promise<void> {
	const byId = await fixtureNumberById(api);
	await expectProgrammer(api, (snapshot) => {
		expect(snapshot.selected.map((id) => byId[id])).toEqual(expected);
	});
}
