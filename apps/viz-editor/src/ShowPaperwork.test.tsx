import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import { ShowPaperwork } from "./ShowPaperwork";
import { documentSession, type DocumentSummary } from "./document/session";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./document/session", () => ({ documentSession: {
	savePaperwork: vi.fn(), readCompanyLogo: vi.fn(), saveLightingDesignerDefault: vi.fn(),
} }));
const document = { showId: "first", name: "Tour", project: "Tour" } as DocumentSummary;
const logo = { mediaType: "image/jpeg" as const, data: "/9j/4AAQ", width: 120, height: 60 };
async function debounce() { await act(async () => { await vi.advanceTimersByTimeAsync(350); }); }
beforeEach(() => {
	vi.useFakeTimers();
	vi.mocked(open).mockReset().mockResolvedValue("/logo.jpg");
	vi.mocked(documentSession.readCompanyLogo).mockReset().mockResolvedValue(logo);
	vi.mocked(documentSession.saveLightingDesignerDefault).mockReset().mockImplementation(async (value) => value);
	vi.mocked(documentSession.savePaperwork).mockReset().mockImplementation(async (draft) => ({ ...document, ...draft }));
});
afterEach(() => vi.useRealTimers());

it("autosaves uploaded and removed logos, while Make Default uses the current contact fields", async () => {
	render(<ShowPaperwork document={document} onDocument={vi.fn()} onError={vi.fn()} />);
	expect(screen.queryByRole("button", { name: "Save project info" })).toBeNull();
	await act(async () => fireEvent.click(screen.getByRole("button", { name: "Upload logo" })));
	expect(screen.getByRole("img", { name: "Company logo" })).toBeVisible();
	fireEvent.change(screen.getByLabelText("Lighting designer email"), { target: { value: "desk@example.com" } });
	fireEvent.change(screen.getByLabelText("Lighting designer phone"), { target: { value: "123" } });
	await act(async () => fireEvent.click(screen.getByRole("button", { name: "Make Default" })));
	expect(documentSession.saveLightingDesignerDefault).toHaveBeenCalledWith(expect.objectContaining({ contactEmail: "desk@example.com", contactPhone: "123", companyLogo: JSON.stringify(logo) }));
	await debounce();
	expect(documentSession.savePaperwork).toHaveBeenLastCalledWith(expect.objectContaining({ companyLogo: JSON.stringify(logo), contactEmail: "desk@example.com" }), "first");
	fireEvent.click(screen.getByRole("button", { name: "Remove logo" }));
	await debounce();
	expect(documentSession.savePaperwork).toHaveBeenLastCalledWith(expect.objectContaining({ companyLogo: "" }), "first");
});

it("shows an actionable retry when an autosave fails", async () => {
	vi.mocked(documentSession.savePaperwork).mockRejectedValueOnce(new Error("Read-only file"));
	render(<ShowPaperwork document={document} onDocument={vi.fn()} onError={vi.fn()} />);
	fireEvent.change(screen.getByLabelText("Project"), { target: { value: "Edited tour" } });
	expect(screen.getByText("Saving project information…")).toBeVisible();
	await debounce();
	expect(screen.getByRole("alert")).toHaveTextContent("Read-only file");
	await act(async () => fireEvent.click(screen.getByRole("button", { name: "Retry saving project information" })));
	expect(screen.queryByRole("alert")).toBeNull();
	expect(documentSession.savePaperwork).toHaveBeenCalledTimes(2);
});


it("does not apply a logo read that finishes after leaving its show", async () => {
	let finish!: (value: typeof logo) => void;
	vi.mocked(documentSession.readCompanyLogo).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
	const { unmount } = render(<ShowPaperwork document={document} onDocument={vi.fn()} onError={vi.fn()} />);
	await act(async () => fireEvent.click(screen.getByRole("button", { name: "Upload logo" })));
	unmount();
	await act(async () => finish(logo));
	await debounce();
	expect(documentSession.savePaperwork).not.toHaveBeenCalled();
});
