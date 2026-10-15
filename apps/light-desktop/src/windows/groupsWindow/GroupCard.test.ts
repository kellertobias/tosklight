import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { GroupSelectionRule } from "../../api/types";
import { defaultPoolPresentation } from "../../features/poolPresentation/poolPresentation";
import { GroupCard, groupReferencePresentation } from "./GroupCard";
import type { Group } from "./model";

describe("groupReferencePresentation", () => {
	it.each<[GroupSelectionRule, string, string]>([
		[{ type: "all" }, "21", "all fixtures"],
		[{ type: "odd" }, "21 % 2", "every 2 fixtures starting at position 1"],
		[{ type: "even" }, "21 % 2 + 1", "every 2 fixtures starting at position 2"],
		[
			{ type: "every_nth", n: 2, offset: 0 },
			"21 % 2",
			"every 2 fixtures starting at position 1",
		],
		[
			{ type: "every_nth", n: 3, offset: 2 },
			"21 % 3 + 2",
			"every 3 fixtures starting at position 3",
		],
	])("shows actual construction for %j", (rule, compact, description) => {
		expect(groupReferencePresentation([{ group_id: "21", rule }])).toEqual({
			compact,
			description: `Group 21, ${description}`,
		});
	});

	it("retains ordered multiple and long source selections with their own rules", () => {
		const id = "12345678901234567890";
		expect(groupReferencePresentation([
			{ group_id: id, rule: { type: "every_nth", n: 2, offset: 1 } },
			{ group_id: "4", rule: { type: "all" } },
		])).toEqual({
			compact: `${id} % 2 + 1, 4`,
			description: `Group ${id}, every 2 fixtures starting at position 2; Group 4, all fixtures`,
		});
	});

	it("has no reference text for explicit membership", () => {
		expect(groupReferencePresentation([])).toEqual({
			compact: "",
			description: "",
		});
	});
});

function markup(body: Group["body"]) {
	return renderToStaticMarkup(createElement(GroupCard, {
		group: { kind: "group", id: "22", revision: 1, updated_at: "", body },
		index: 21,
		poolSlotId: "22",
		knownFixtureIds: new Set<string>(),
		capabilities: new Map<string, Set<string>>(),
		selected: false,
		partiallySelected: false,
		selectedFixtureCount: 0,
		storeArmed: false,
		updateArmed: false,
		setTarget: false,
		mutationOperation: null,
		poolPresentation: defaultPoolPresentation(),
		showId: "show",
		surfaceKey: "group",
		beginHold: () => {},
		cancelHold: () => {},
		consumeHold: () => false,
		openSettings: () => {},
		dereference: () => {},
		select: () => {},
	}));
}

describe("Group source presentation in real PoolCard", () => {
	it("keeps primary name and number while rendering a monochrome link and complete construction", () => {
		const html = markup({
			name: "Front odd",
			fixtures: [],
			source: {
				type: "references",
				references: [
					{ group_id: "21", rule: { type: "every_nth", n: 2, offset: 1 } },
				],
			},
		});
		expect(html).toContain('class="number">22</span>');
		expect(html).toContain('class="pool-card-name">Front odd</b>');
		expect(html).toContain('stroke="currentColor"');
		expect(html).toContain('class="group-reference-expression">21 % 2 + 1</span>');
		expect(html).toContain('title="Group 21, every 2 fixtures starting at position 2"');
		expect(html).not.toContain("Ref:");
	});

	it("shows legacy derivation but does not show it over canonical explicit membership", () => {
		const body: Group["body"] = {
			name: "Legacy",
			fixtures: [],
			derived_from: { source_group_id: "9", rule: { type: "odd" } },
		};
		expect(markup(body)).toContain('class="group-reference-expression">9 % 2</span>');
		expect(markup({ ...body, source: { type: "explicit", fixture_ids: [] } }))
			.not.toContain("group-reference-construction");
	});
});
