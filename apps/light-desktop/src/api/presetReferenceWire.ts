import { arrayAt, enumAt, integerAt, recordAt, stringAt } from "./playbackWirePrimitives";
import type { PresetValueReference } from "./types/playback";
import { WireValidationError } from "./wireValidation";

export function presetInstanceIdAt(value: unknown, path: string): string {
	const id = stringAt(value, path);
	if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id) || /^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(id))
		throw new WireValidationError(path, "non-nil Preset instance UUID", value);
	return id;
}

export function decodePresetReference(value: unknown, path: string, attribute: string): PresetValueReference {
	const ref = recordAt(value, path);
	const owner = recordAt(ref.source_owner, `${path}.source_owner`);
	const type = enumAt(owner.type, `${path}.source_owner.type`, ["universal", "fixture", "group"]);
	const source_attribute = stringAt(ref.source_attribute, `${path}.source_attribute`);
	if (source_attribute !== attribute)
		throw new WireValidationError(`${path}.source_attribute`, attribute, source_attribute);
	const source_owner: PresetValueReference["source_owner"] = type === "universal"
		? { type }
		: type === "fixture"
			? { type, fixture_id: stringAt(owner.fixture_id, `${path}.source_owner.fixture_id`) }
			: { type, group_id: stringAt(owner.group_id, `${path}.source_owner.group_id`) };
	let sample_rank: [number, number] | undefined;
	if (ref.sample_rank != null) {
		const pair = arrayAt(ref.sample_rank, `${path}.sample_rank`);
		if (pair.length !== 2) throw new WireValidationError(`${path}.sample_rank`, "rank/count pair", pair);
		const rank = integerAt(pair[0], `${path}.sample_rank[0]`);
		const count = integerAt(pair[1], `${path}.sample_rank[1]`);
		if (count === 0 || rank >= count) throw new WireValidationError(`${path}.sample_rank`, "rank below positive count", pair);
		sample_rank = [rank, count];
	}
	return { preset_instance_id: presetInstanceIdAt(ref.preset_instance_id, `${path}.preset_instance_id`), source_owner, source_attribute,
		...(sample_rank ? { sample_rank } : {}),
		...(ref.member_fixture != null ? { member_fixture: stringAt(ref.member_fixture, `${path}.member_fixture`) } : {}),
	};
}
