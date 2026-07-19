import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	SelectiveImportCatalog,
	SelectiveImportOutcome,
	SelectiveImportPreview,
} from "../../api/selectiveImportModels";
import { SelectiveShowImportModal } from "./SelectiveShowImportModal";

const target = { id: "target", name: "Current Show" } as never;
const source = { id: "source", name: "Tour Source" } as never;
const catalog: SelectiveImportCatalog = {
	sourceShowId: "source",
	sourceShowName: "Tour Source",
	sourceRevision: 4,
	objects: [
		{
			key: { kind: "group", id: "front" },
			objectRevision: 2,
			displayName: "Front Wash",
		},
	],
};

function preview(canApply: boolean): SelectiveImportPreview {
	return {
		sourceShowId: "source",
		targetShowId: "target",
		sourceRevision: 4,
		targetRevision: 9,
		objects: [
			{
				source: { kind: "group", id: "front" },
				destination: { kind: "group", id: "front" },
				action: canApply ? "replace_destination" : "blocked_conflict",
			},
		],
		dependencies: [
			{
				owner: { kind: "group", id: "front" },
				dependency: { kind: "patched_fixture", id: "fixture-a" },
				disposition: "bound_to_destination",
			},
		],
		conflicts: canApply
			? [{ key: { kind: "group", id: "front" }, resolution: "replace_destination" }]
			: [{ key: { kind: "group", id: "front" }, resolution: null }],
		profiles: [],
		managedAssets: [],
		blockers: canApply
			? []
			: [{ type: "object_conflict", summary: "Object Conflict: group/front" }],
		canApply,
	};
}

function outcome(): SelectiveImportOutcome {
	return {
		requestId: "request",
		correlationId: "correlation",
		changed: true,
		showId: "target",
		showRevision: 10,
		eventSequence: 7,
		objectChanges: [
			{
				key: { kind: "group", id: "front" },
				objectRevision: 3,
				body: { name: "Front Wash" },
			},
		],
		outcomes: preview(true).objects,
		profileChanges: [],
		managedAssets: [],
	};
}

afterEach(() => cleanup());

describe("SelectiveShowImportModal", () => {
	it("requires an explicit preview and conflict resolution before atomic apply", async () => {
		const loadCatalog = vi.fn().mockResolvedValue(catalog);
		const previewImport = vi
			.fn()
			.mockResolvedValueOnce(preview(false))
			.mockResolvedValueOnce(preview(true));
		const applyImport = vi.fn().mockResolvedValue(outcome());
		render(
			<SelectiveShowImportModal
				activeShow={target}
				shows={[target, source]}
				onClose={vi.fn()}
				loadCatalog={loadCatalog}
				previewImport={previewImport}
				applyImport={applyImport}
			/>,
		);

		fireEvent.change(screen.getByLabelText("Source show"), {
			target: { value: "source" },
		});
		const object = await screen.findByLabelText(/Front Wash/);
		fireEvent.click(object);
		fireEvent.click(screen.getByRole("button", { name: "Preview Import" }));

		const details = await screen.findByLabelText("Selective Show Import preview");
		expect(details).toHaveTextContent("Bound To Destination");
		expect(details).toHaveTextContent("Object Conflict: group/front");
		expect(screen.getByRole("button", { name: "Apply as One Show Revision" })).toBeDisabled();

		fireEvent.change(screen.getByLabelText("Resolve group front"), {
			target: { value: "replace_destination" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Update Preview" }));
		await waitFor(() =>
			expect(screen.getByText("None — ready to apply.")).toBeVisible(),
		);
		fireEvent.click(screen.getByRole("button", { name: "Apply as One Show Revision" }));

		await waitFor(() => expect(applyImport).toHaveBeenCalledOnce());
		expect(applyImport.mock.calls[0][2]).toMatchObject({
			expectedSourceRevision: 4,
			expectedTargetRevision: 9,
			selectedObjects: [{ kind: "group", id: "front" }],
			conflictResolutions: [
				{
					key: { kind: "group", id: "front" },
					resolution: "replace_destination",
				},
			],
		});
		expect(await screen.findByRole("dialog", { name: "Partial Show Load complete" })).toHaveTextContent(
			"one show revision",
		);
	});

	it("keeps cancellation available during preview but locks the modal during atomic apply", async () => {
		let completeApply!: (value: SelectiveImportOutcome) => void;
		const applyImport = vi.fn(
			() => new Promise<SelectiveImportOutcome>((resolve) => { completeApply = resolve; }),
		);
		const onClose = vi.fn();
		render(
			<SelectiveShowImportModal
				activeShow={target}
				shows={[target, source]}
				onClose={onClose}
				loadCatalog={vi.fn().mockResolvedValue(catalog)}
				previewImport={vi.fn().mockResolvedValue(preview(true))}
				applyImport={applyImport}
			/>,
		);
		fireEvent.change(screen.getByLabelText("Source show"), { target: { value: "source" } });
		fireEvent.click(await screen.findByLabelText(/Front Wash/));
		fireEvent.click(screen.getByRole("button", { name: "Preview Import" }));
		await screen.findByText("None — ready to apply.");
		fireEvent.click(screen.getByRole("button", { name: "Apply as One Show Revision" }));

		expect(await screen.findByText(/cannot be cancelled/)).toBeVisible();
		expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
		expect(screen.getByLabelText("Resolve group front")).toBeDisabled();
		fireEvent.click(within(screen.getByRole("dialog", { name: "Partial Show Load" })).getByRole("button", { name: "Close Partial Show Load" }));
		expect(onClose).not.toHaveBeenCalled();

		completeApply(outcome());
		await screen.findByRole("dialog", { name: "Partial Show Load complete" });
	});

	it("invalidates an older preview when refreshing it fails", async () => {
		const previewImport = vi
			.fn()
			.mockResolvedValueOnce(preview(true))
			.mockRejectedValueOnce(new Error("source show changed after preview"));
		render(
			<SelectiveShowImportModal
				activeShow={target}
				shows={[target, source]}
				onClose={vi.fn()}
				loadCatalog={vi.fn().mockResolvedValue(catalog)}
				previewImport={previewImport}
				applyImport={vi.fn().mockResolvedValue(outcome())}
			/>,
		);
		fireEvent.change(screen.getByLabelText("Source show"), { target: { value: "source" } });
		fireEvent.click(await screen.findByLabelText(/Front Wash/));
		fireEvent.click(screen.getByRole("button", { name: "Preview Import" }));
		await screen.findByText("None — ready to apply.");
		expect(screen.getByRole("button", { name: "Apply as One Show Revision" })).toBeEnabled();

		fireEvent.click(screen.getByRole("button", { name: "Update Preview" }));

		expect(await screen.findByRole("alert")).toHaveTextContent("source show changed after preview");
		expect(screen.getByRole("button", { name: "Apply as One Show Revision" })).toBeDisabled();
	});

	it("removes a cleared conflict choice instead of sending an empty wire enum", async () => {
		const previewImport = vi
			.fn()
			.mockResolvedValueOnce(preview(false))
			.mockResolvedValueOnce(preview(true))
			.mockResolvedValueOnce(preview(false));
		render(
			<SelectiveShowImportModal
				activeShow={target}
				shows={[target, source]}
				onClose={vi.fn()}
				loadCatalog={vi.fn().mockResolvedValue(catalog)}
				previewImport={previewImport}
				applyImport={vi.fn().mockResolvedValue(outcome())}
			/>,
		);
		fireEvent.change(screen.getByLabelText("Source show"), { target: { value: "source" } });
		fireEvent.click(await screen.findByLabelText(/Front Wash/));
		fireEvent.click(screen.getByRole("button", { name: "Preview Import" }));
		const resolution = await screen.findByLabelText("Resolve group front");
		fireEvent.change(resolution, { target: { value: "replace_destination" } });
		fireEvent.click(screen.getByRole("button", { name: "Update Preview" }));
		await screen.findByText("None — ready to apply.");

		fireEvent.change(resolution, { target: { value: "" } });

		expect(screen.getByRole("button", { name: "Apply as One Show Revision" })).toBeDisabled();
		fireEvent.click(screen.getByRole("button", { name: "Update Preview" }));
		await waitFor(() => expect(previewImport).toHaveBeenCalledTimes(3));
		expect(previewImport.mock.calls[2][2].conflictResolutions).toEqual([]);
	});
});
