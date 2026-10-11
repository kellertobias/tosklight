import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { decodeShowObject } from "../../api/showObjectWire";
import { decodeShowObjectsEventMessage } from "../../api/showObjectsWire";
import {
	ShowObjectsStateProvider,
	useMacros,
	useTimecodes,
} from "./ShowObjectsState";
import { ShowObjectsStore } from "./store";
const SHOW = "show-1";
function event(
	kind: "macro" | "timecode",
	sequence: number,
	name: string | null,
) {
	return {
		type: "event",
		event: {
			sequence,
			payload: {
				type: "show_objects_changed",
				change: {
					show_id: SHOW,
					show_revision: sequence,
					changes: [
						{
							kind,
							object_id: "object-1",
							object_revision: sequence,
							deleted: name === null,
							body:
								name === null
									? null
									: {
											id: "object-1",
											number: 7,
											name,
											extra: { preserved: true },
										},
						},
					],
				},
			},
		},
	};
}
function apply(
	store: ShowObjectsStore,
	kind: "macro" | "timecode",
	sequence: number,
	name: string | null,
) {
	const decoded = decodeShowObjectsEventMessage(event(kind, sequence, name));
	if (decoded?.type !== "event") throw new Error("Missing authored event");
	store.applyChange(decoded.change);
}
function Options() {
	const macros = useMacros();
	const timecodes = useTimecodes();
	return (
		<div>
			{[...macros, ...timecodes].map((object) => (
				<span key={object.kind + object.id}>{object.body.name}</span>
			))}
		</div>
	);
}
afterEach(cleanup);
describe("authoritative automation chooser metadata", () => {
	it.each([
		"macro",
		"timecode",
	] as const)("hydrates empty %s collections and handles versioned create/update/delete/reset without resurrecting stale entries", (kind) => {
		const store = new ShowObjectsStore();
		store.reset(SHOW);
		store.setCollection(SHOW, kind, [], 1);
		render(
			<ShowObjectsStateProvider store={store}>
				<Options />
			</ShowObjectsStateProvider>,
		);
		act(() => apply(store, kind, 2, "Opening"));
		expect(screen.getByText("Opening")).toBeVisible();
		act(() => apply(store, kind, 3, "Updated"));
		expect(screen.getByText("Updated")).toBeVisible();
		expect(screen.queryByText("Opening")).toBeNull();
		act(() => apply(store, kind, 4, null));
		expect(screen.queryByText("Updated")).toBeNull();
		act(() => apply(store, kind, 3, "Delayed"));
		expect(screen.queryByText("Delayed")).toBeNull();
		act(() => apply(store, kind, 5, "Current"));
		expect(screen.getByText("Current")).toBeVisible();
		act(() => store.reset("show-2"));
		expect(screen.queryByText("Current")).toBeNull();
		act(() => apply(store, kind, 6, "Previous show"));
		expect(screen.queryByText("Previous show")).toBeNull();
		expect(store.getSnapshot().groups).toEqual([]);
		expect(store.getSnapshot().cueLists).toEqual([]);
	});
	it.each([
		"macro",
		"timecode",
	] as const)("strictly validates %s metadata identity/number/name and preserves extras", (kind) => {
		const body = {
			id: "object-1",
			number: 7,
			name: "Opening",
			extra: { preserved: true },
		};
		const object = (candidate: unknown) => ({
			kind,
			id: "object-1",
			revision: 1,
			updated_at: "",
			body: candidate,
		});
		expect(decodeShowObject(object(body), kind).body).toEqual(body);
		for (const patch of [
			{ id: "different" },
			{ number: 0 },
			{ number: "7" },
			{ name: null },
		])
			expect(() =>
				decodeShowObject(object({ ...body, ...patch }), kind),
			).toThrow();
	});
});

it("accepts the full canonical Timecode u32 number range while keeping Macro bounds", () => {
	const object = (kind: "macro" | "timecode", number: number) => ({
		kind,
		id: "object-1",
		revision: 1,
		updated_at: "",
		body: { id: "object-1", number, name: "Large number" },
	});
	for (const number of [10000, 4294967295])
		expect(
			decodeShowObject(object("timecode", number), "timecode").body.number,
		).toBe(number);
	expect(() =>
		decodeShowObject(object("timecode", 4294967296), "timecode"),
	).toThrow();
	expect(() => decodeShowObject(object("macro", 10000), "macro")).toThrow();
});
