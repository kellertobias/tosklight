import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CadAnnotation } from "./annotations";
import { CadTextInfo } from "./CadTextInfo";
import { CadToolContext, type CadTools } from "./cadTools";

const note: CadAnnotation = {
	id: "note",
	view: "top_down",
	kind: "text",
	points: [[-3000, 1500]],
	closed: false,
	text: "Stage left",
	textHeightMillimetres: 250,
};

function show(annotation: CadAnnotation) {
	const change = vi.fn().mockResolvedValue(undefined);
	render(
		<CadToolContext.Provider value={{ change } as unknown as CadTools}>
			<CadTextInfo annotation={annotation} />
		</CadToolContext.Provider>,
	);
	const commit = (label: string, value: string) => {
		const field = screen.getByLabelText(label);
		fireEvent.change(field, { target: { value } });
		fireEvent.keyDown(field, { key: "Enter" });
	};
	return { change, commit };
}

describe("Info for picked text", () => {
	it("moves the text to a typed position and rewords and resizes it, each as one change", () => {
		const { change, commit } = show(note);
		expect(screen.getByLabelText("X")).toHaveValue("-3");
		expect(screen.getByLabelText("Y")).toHaveValue("1.5");
		commit("X", "-2");
		expect(change).toHaveBeenLastCalledWith({ ...note, points: [[-2000, 1500]] });
		commit("Y", "0,5");
		expect(change).toHaveBeenLastCalledWith({ ...note, points: [[-3000, 500]] });
		commit("Text", "Stage right");
		expect(change).toHaveBeenLastCalledWith({ ...note, text: "Stage right" });
		commit("Height", "0.4");
		expect(change).toHaveBeenLastCalledWith({ ...note, textHeightMillimetres: 400 });
	});

	it("names the position by the page's axes on an elevation", () => {
		show({ ...note, view: "front_to_back" });
		expect(screen.getByLabelText("Across")).toHaveValue("-3");
		expect(screen.getByLabelText("Position height")).toHaveValue("1.5");
	});
});

describe("Info's title for picked text", () => {
	it("carries no Generic and Placement tabs, which divide only an element's Info", async () => {
		const { CadSidePanels } = await import("./CadSidePanels");
		render(
			<CadToolContext.Provider value={{ annotations: [note], selectedTextId: "note" } as unknown as CadTools}>
				<CadSidePanels
					panel={null}
					scene={{ sceneRevision: 1, entities: [], selectedIds: [] } as never}
					tools={{ annotations: [note], selectedTextId: "note", change: vi.fn() } as unknown as CadTools}
					underlayState={{} as never}
					defaultView="top_down"
					documentKey={null}
					printPages={{} as never}
					exporting={false}
					onExport={vi.fn()}
					onSelect={vi.fn()}
					focusedEntityId={null}
					onFocusEntity={vi.fn()}
					onError={vi.fn()}
					onNotice={vi.fn()}
				/>
			</CadToolContext.Provider>,
		);
		expect(screen.getByLabelText("Text")).toHaveValue("Stage left");
		expect(screen.queryByRole("tab", { name: "Generic" })).toBeNull();
		expect(screen.queryByRole("tab", { name: "Placement" })).toBeNull();
	});
});
