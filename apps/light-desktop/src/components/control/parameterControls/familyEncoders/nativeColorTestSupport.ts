import type {
	NativeColorControlDescriptor,
	NativeColorPagesSnapshot,
} from "../../../../api/nativeColorModels";
import { FIXTURE_A, FIXTURE_B } from "./familyEncoderTestSupport";

/** Shared TL-554 fixtures (not production code): a verified RGBW+ reference head. */

export const HEAD = "33333333-3333-4333-8333-333333333333";

const WIDTHS = [
	["8bit", 255],
	["16bit", 65_535],
	["24bit", 16_777_215],
	["32bit", 4_294_967_295],
] as const;

export function nativeControl(
	index: number,
	overrides: Partial<NativeColorControlDescriptor> = {},
): NativeColorControlDescriptor {
	const [resolution, rawMax] = WIDTHS[index % 4];
	const channel = `c${index}000000-0000-4000-8000-000000000000`.slice(0, 36);
	return {
		id: `native.${channel}`,
		channel_id: channel,
		label: `Emitter ${index + 1}`,
		raw_max: rawMax,
		resolution,
		ultraviolet: false,
		functions: [
			{
				function_id: `f${index}000000-0000-4000-8000-000000000000`.slice(0, 36),
				label: `Emitter ${index + 1}`,
				raw_from: 0,
				raw_to: rawMax,
				continuous: true,
			},
		],
		...overrides,
	};
}

/** A wheel: three discrete slot functions on one 8-bit channel. */
export function wheelControl(index: number): NativeColorControlDescriptor {
	return nativeControl(index, {
		label: "Color wheel",
		raw_max: 255,
		resolution: "8bit",
		functions: ["Open", "Red", "Blue"].map((label, slot) => ({
			function_id: `e${index}${slot}00000-0000-4000-8000-000000000000`.slice(0, 36),
			label,
			raw_from: slot * 10,
			raw_to: slot * 10 + 9,
			continuous: false,
		})),
	});
}

/** Native pages of `count` controls: eight on pages 3/4, the rest overflow. */
export function nativePages(count: number, values = true): NativeColorPagesSnapshot {
	const controls = Array.from({ length: count }, (_, index) => nativeControl(index));
	const onPages = controls.slice(0, 8);
	const pages = [onPages.slice(0, 4), onPages.slice(4, 8)]
		.filter((page) => page.length)
		.map((page, index) => ({
			number: 3 + index,
			controls: [...page, ...Array(4 - page.length).fill(null)],
		}));
	return {
		semantic: true,
		show_revision: 1,
		fixture_ids: [FIXTURE_A, FIXTURE_B],
		reference: {
			fixture_id: FIXTURE_A,
			fixture_number: 101,
			fixture_name: "Wash",
			head_id: HEAD,
			head_name: "Main",
			chosen: false,
			identity: {},
		},
		candidates: [
			{ fixture_id: FIXTURE_A, fixture_number: 101, fixture_name: "Wash", head_id: HEAD, head_name: "Main" },
			{ fixture_id: FIXTURE_B, fixture_number: 102, fixture_name: "Wash", head_id: HEAD, head_name: "Main" },
		],
		pages,
		overflow: controls.slice(8),
		fixtures: [
			{ fixture_id: FIXTURE_A, replay: "exact" },
			{ fixture_id: FIXTURE_B, replay: "fallback" },
		],
		...(values
			? {
					values: {
						frame: { generation: 1, sequence: 1, sampled_at: "1970-01-01T00:00:00Z" },
						controls: controls.map((control) => ({
							channel_id: control.channel_id,
							function_id: control.functions[0].function_id,
							raw: Math.floor(control.raw_max / 2),
						})),
					},
				}
			: {}),
	} as unknown as NativeColorPagesSnapshot;
}
