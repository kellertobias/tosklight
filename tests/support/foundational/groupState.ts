import type { ApiDriver } from "../../../apps/control-ui/e2e/bench/api";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import {
	fixtureIdsByNumber,
	fixtureNumberById,
	object,
	objects,
	putObject,
} from "./apiState";
import type { VersionedObject } from "./contracts";

export async function expectGroup(
	api: ApiDriver,
	id: string,
	assertion: (group: VersionedObject) => void,
): Promise<void> {
	await expect(async () => {
		const group = (await objects(api, "group")).find(
			(item) => item.id === id,
		);
		expect(group).toBeDefined();
		assertion(group!);
	}).toPass({ timeout: 2_000 });
}

export async function expectGroupMissing(
	api: ApiDriver,
	id: string,
): Promise<void> {
	await expect
		.poll(
			async () => (await objects(api, "group")).some((item) => item.id === id),
			{ timeout: 2_000 },
		)
		.toBe(false);
}

export async function expectGroupNumbers(
	api: ApiDriver,
	id: string,
	expected: number[],
): Promise<void> {
	const byId = await fixtureNumberById(api);
	await expectGroup(api, id, (group) =>
		expect(group.body.fixtures.map((fixture: string) => byId[fixture])).toEqual(
			expected,
		),
	);
}

export async function setGroupByNumbers(
	api: ApiDriver,
	id: string,
	name: string,
	numbers: number[],
): Promise<void> {
	const byNumber = await fixtureIdsByNumber(api);
	const existing = (await objects(api, "group")).find(
		(group) => group.id === id,
	);
	await putObject(
		api,
		"group",
		id,
		{
			...(existing?.body ?? {}),
			id,
			name,
			fixtures: numbers.map((number) => byNumber[number]),
			derived_from: null,
			frozen_from: null,
			programming: existing?.body.programming ?? {},
			master: existing?.body.master ?? 1,
			playback_fader: existing?.body.playback_fader ?? null,
		},
		existing?.revision ?? 0,
	);
}

export async function overwriteGroupByNumbers(
	api: ApiDriver,
	id: string,
	numbers: number[],
): Promise<void> {
	const byNumber = await fixtureIdsByNumber(api);
	const existing = await object(api, "group", id);
	await putObject(
		api,
		"group",
		id,
		{
			...existing.body,
			fixtures: numbers.map((number) => byNumber[number]),
			derived_from: null,
			frozen_from: null,
		},
		existing.revision,
	);
}
