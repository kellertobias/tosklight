import type {
	NativeColorControlDescriptor,
	NativeColorPagesSnapshot,
} from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";
import { MAX_FAMILY_ENCODER_FIXTURES } from "./familyEncoderPagesWire";

/**
 * `GET /api/v2/programming/color/native-pages` (TL-554): Direct Color pages 3/4, the modal
 * overflow and the clearly identified reference head of one ordered selection. Reading it is
 * inert on the server. The decoder checks every field the encoders and the modal rely on and
 * tolerates unknown fields (api-rules §5). Raw values are full width (up to 32 bit).
 */

export const NATIVE_COLOR_PAGES_PATH = "/api/v2/programming/color/native-pages";

export interface NativeColorReferenceChoice {
	fixtureId: string;
	headId?: string | null;
}

export function nativeColorPagesPath(
	fixtureIds: readonly string[],
	reference?: NativeColorReferenceChoice | null,
) {
	const owners = fixtureIds
		.slice(0, MAX_FAMILY_ENCODER_FIXTURES)
		.map(encodeURIComponent)
		.join(",");
	const chosen = reference
		? `&reference=${encodeURIComponent(reference.fixtureId)}${
				reference.headId ? `&head=${encodeURIComponent(reference.headId)}` : ""
			}`
		: "";
	return `${NATIVE_COLOR_PAGES_PATH}?fixture_ids=${owners}${chosen}`;
}

const RESOLUTIONS = ["8bit", "16bit", "24bit", "32bit"] as const;
const UNAVAILABLE = ["contract", "no_verified_head"] as const;
const REPLAY = ["exact", "fallback"] as const;
const U32_MAX = 4_294_967_295;

function rawAt(value: unknown, path: string) {
	const raw = integerAt(value, path);
	if (raw < 0 || raw > U32_MAX)
		throw new WireValidationError(path, "0..4294967295", value);
	return raw;
}

function decodeControl(value: unknown, path: string): NativeColorControlDescriptor {
	const control = recordAt(value, path);
	stringAt(control.id, `${path}.id`);
	stringAt(control.channel_id, `${path}.channel_id`);
	stringAt(control.label, `${path}.label`);
	rawAt(control.raw_max, `${path}.raw_max`);
	enumAt(control.resolution, `${path}.resolution`, RESOLUTIONS);
	booleanAt(control.ultraviolet, `${path}.ultraviolet`);
	arrayAt(control.functions, `${path}.functions`).forEach((entry, index) => {
		const at = `${path}.functions[${index}]`;
		const fn = recordAt(entry, at);
		stringAt(fn.function_id, `${at}.function_id`);
		stringAt(fn.label, `${at}.label`);
		rawAt(fn.raw_from, `${at}.raw_from`);
		rawAt(fn.raw_to, `${at}.raw_to`);
		booleanAt(fn.continuous, `${at}.continuous`);
	});
	return control as unknown as NativeColorControlDescriptor;
}

export function decodeNativeColorPagesSnapshot(value: unknown): NativeColorPagesSnapshot {
	const snapshot = recordAt(value, "$");
	booleanAt(snapshot.semantic, "$.semantic");
	arrayAt(snapshot.fixture_ids, "$.fixture_ids").forEach((id, index) =>
		stringAt(id, `$.fixture_ids[${index}]`),
	);
	if (snapshot.unavailable != null)
		enumAt(snapshot.unavailable, "$.unavailable", UNAVAILABLE);
	if (snapshot.reference != null) {
		const reference = recordAt(snapshot.reference, "$.reference");
		stringAt(reference.fixture_id, "$.reference.fixture_id");
		stringAt(reference.head_id, "$.reference.head_id");
		stringAt(reference.fixture_name, "$.reference.fixture_name");
		booleanAt(reference.chosen, "$.reference.chosen");
		recordAt(reference.identity, "$.reference.identity");
	}
	arrayAt(snapshot.candidates, "$.candidates").forEach((entry, index) => {
		const candidate = recordAt(entry, `$.candidates[${index}]`);
		stringAt(candidate.fixture_id, `$.candidates[${index}].fixture_id`);
		stringAt(candidate.head_id, `$.candidates[${index}].head_id`);
	});
	arrayAt(snapshot.pages, "$.pages").forEach((entry, index) => {
		const page = recordAt(entry, `$.pages[${index}]`);
		integerAt(page.number, `$.pages[${index}].number`);
		arrayAt(page.controls, `$.pages[${index}].controls`).forEach((control, slot) => {
			if (control !== null)
				decodeControl(control, `$.pages[${index}].controls[${slot}]`);
		});
	});
	arrayAt(snapshot.overflow, "$.overflow").forEach((control, index) =>
		decodeControl(control, `$.overflow[${index}]`),
	);
	arrayAt(snapshot.fixtures, "$.fixtures").forEach((entry, index) => {
		const fixture = recordAt(entry, `$.fixtures[${index}]`);
		stringAt(fixture.fixture_id, `$.fixtures[${index}].fixture_id`);
		enumAt(fixture.replay, `$.fixtures[${index}].replay`, REPLAY);
	});
	if (snapshot.values != null) {
		const values = recordAt(snapshot.values, "$.values");
		arrayAt(values.controls, "$.values.controls").forEach((entry, index) => {
			const at = `$.values.controls[${index}]`;
			const control = recordAt(entry, at);
			stringAt(control.channel_id, `${at}.channel_id`);
			stringAt(control.function_id, `${at}.function_id`);
			rawAt(control.raw, `${at}.raw`);
		});
	}
	return snapshot as unknown as NativeColorPagesSnapshot;
}
