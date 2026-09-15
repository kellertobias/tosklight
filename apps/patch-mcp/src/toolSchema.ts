/** What every tool is made of, and the input shapes several tools share. */

import type { PatchBackend, PatchedFixture } from "./backend";

export interface Tool {
	name: string;
	description: string;
	inputSchema: {
		type: "object";
		properties: Record<string, unknown>;
		required?: string[];
	};
	run(desk: PatchBackend, input: Record<string, any>): Promise<unknown>;
}

export const fixtureNumber = {
	type: ["number", "string"],
	description:
		"The fixture number an operator would say out loud, or a Venue object's 0.N number (pass \"0.10\" as a string).",
};

/** A gel, in the two shapes the patch accepts. */
export const gel = {
	type: "object",
	description:
		"Open white when omitted. A built-in gel names a catalog entry; a custom gel carries its own sRGB colour.",
	properties: {
		catalog_id: { type: "string" },
		entry_id: { type: "string" },
		name: { type: "string" },
		color_srgb: {
			type: "string",
			description: "`#rrggbb`, for a custom gel.",
		},
	},
};

export function appearanceWithGel(
	fixture: PatchedFixture,
	input: Record<string, any>,
): Record<string, unknown> {
	const appearance = {
		...((fixture.installed_appearance as Record<string, unknown>) ?? {}),
	};
	if (input.gel) {
		appearance.gel = input.gel.catalog_id
			? {
					type: "built_in",
					catalog_id: input.gel.catalog_id,
					entry_id: input.gel.entry_id,
				}
			: {
					type: "custom",
					name: input.gel.name ?? "Custom",
					color_srgb: input.gel.color_srgb,
					note: null,
				};
	}
	return appearance;
}
