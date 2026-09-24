import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { CadAnnotation } from "./annotations";

const note: CadAnnotation = {
	id: "note",
	view: "top_down",
	kind: "text",
	points: [[-3000, 0]],
	closed: false,
	text: "Stage left",
	textHeightMillimetres: 250,
};

const session = vi.hoisted(() => ({
	all: vi.fn(),
	change: vi.fn(),
	onDelta: vi.fn(),
}));
vi.mock("./annotations", () => ({ annotationSession: session }));

import { CadToolProvider, useCadTools } from "./cadTools";

describe("changing drawn items", () => {
	it("draws moved text from the show's answer at once, without waiting for the delta", async () => {
		session.all.mockResolvedValue([note]);
		// No delta ever arrives: the answer to the change alone must move the text and its anchor.
		session.onDelta.mockResolvedValue(() => undefined);
		session.change.mockImplementation(async (annotation: CadAnnotation) => annotation);
		const wrapper = ({ children }: { children: ReactNode }) => (
			<CadToolProvider documentKey="show" onAdd={vi.fn()}>
				{children}
			</CadToolProvider>
		);
		const { result } = renderHook(() => useCadTools(), { wrapper });
		await waitFor(() => expect(result.current.annotations).toEqual([note]));
		await act(() => result.current.change({ ...note, points: [[-2000, 0]] }));
		expect(result.current.annotations).toEqual([{ ...note, points: [[-2000, 0]] }]);
	});
});
