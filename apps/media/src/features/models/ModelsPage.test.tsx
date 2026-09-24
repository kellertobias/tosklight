import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ModelSlotView } from "../../shared/api/generated/media-wire";
import { resetResources } from "../../shared/api/resource";
import { ModelsPage } from "./ModelsPage";

afterEach(() => {
	vi.unstubAllGlobals();
	resetResources();
});

const cube: ModelSlotView = {
	slot: 1,
	name: "Stage cube",
	vertices: 24,
	triangles: 12,
	builtin: null,
	status: "ready",
	detail: null,
};

const BUILT_INS: ModelSlotView[] = (
	[
		["plane", "Plane", 4, 2],
		["cube", "Cube", 24, 12],
		["sphere", "Sphere", 1225, 2208],
		["cylinder", "Cylinder", 198, 192],
		["pyramid", "Pyramid", 16, 6],
	] as const
).map(([builtin, name, vertices, triangles], index) => ({
	slot: index + 1,
	name,
	vertices,
	triangles,
	builtin,
	status: "ready",
	detail: null,
}));

/** An upload the test finishes by hand, so every intermediate state is observable. */
class FakeUpload {
	static last: FakeUpload | undefined;
	url = "";
	body: FormData | undefined;
	status = 0;
	responseText = "";
	upload: { onprogress: ((event: ProgressEvent) => void) | null } = {
		onprogress: null,
	};
	onload: (() => void) | null = null;
	onerror: (() => void) | null = null;

	open(_method: string, url: string) {
		this.url = url;
	}

	send(body: FormData) {
		this.body = body;
		FakeUpload.last = this;
	}

	progress(loaded: number, total: number) {
		this.upload.onprogress?.({
			lengthComputable: true,
			loaded,
			total,
		} as ProgressEvent);
	}

	respond(status: number, body: unknown) {
		this.status = status;
		this.responseText = JSON.stringify(body);
		this.onload?.();
	}
}

function stubModels(initial: ModelSlotView[]) {
	const models = structuredClone(initial);
	const writes: Array<{ path: string; body: Record<string, unknown> }> = [];
	FakeUpload.last = undefined;
	vi.stubGlobal("XMLHttpRequest", FakeUpload);
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const path = String(input);
			if (path === "/api/v2/models" && !init?.method)
				return Response.json(models);
			const match = /^\/api\/v2\/models\/(\d+)\/update$/u.exec(path);
			if (match && init?.method === "POST") {
				const slot = Number(match[1]);
				const body = JSON.parse(String(init.body)) as Record<string, unknown>;
				writes.push({ path, body });
				const index = models.findIndex((model) => model.slot === slot);
				if (body.clear) {
					models.splice(index, 1);
					return Response.json({ slot, assigned: false });
				}
				if (body.builtin) {
					const template = BUILT_INS.find(
						(model) => model.builtin === body.builtin,
					);
					if (!template) throw new Error(`unknown built-in ${body.builtin}`);
					const assigned = { ...template, slot };
					if (index < 0) models.push(assigned);
					else models[index] = assigned;
					return Response.json(assigned);
				}
				models[index] = { ...models[index], name: String(body.name) };
				return Response.json(models[index]);
			}
			return Response.json(
				{ code: "not-found", message: path },
				{ status: 404 },
			);
		}),
	);
	return { models, writes };
}

function chooseFile(container: HTMLElement, name: string) {
	const input = container.querySelector<HTMLInputElement>('input[type="file"]');
	if (!input) throw new Error("no model file input");
	const file = new File([new Uint8Array([0x67, 0x6c, 0x54, 0x46])], name, {
		type: "model/gltf-binary",
	});
	fireEvent.change(input, { target: { files: [file] } });
}

describe("the 3D model library", () => {
	it("shows all 255 slots under a Models tab with what each holds", async () => {
		stubModels([
			cube,
			{
				slot: 4,
				name: "Broken",
				vertices: 3,
				triangles: 1,
				builtin: null,
				status: "unloadable",
				detail: "cannot read model-004.glb",
			},
		]);
		const { container } = render(<ModelsPage />);

		expect(await screen.findByText("2/255 assigned")).toBeInTheDocument();
		expect(
			container.querySelectorAll(".media-models-pool-grid .pool-card"),
		).toHaveLength(255);
		expect(screen.getByRole("tablist")).toHaveTextContent(
			"MediaVisualizersTextEffectsModels",
		);
		expect(screen.getByText("12 triangles")).toBeInTheDocument();
		expect(screen.getByText("Cannot load")).toBeInTheDocument();
	});

	it("reports upload progress, then the import, then the assigned model", async () => {
		const { models } = stubModels([]);
		const { container } = render(<ModelsPage />);
		await screen.findByText("0/255 assigned");

		chooseFile(container, "Stage cube.glb");
		const upload = FakeUpload.last;
		expect(upload?.url).toMatch(/^\/api\/v2\/models\/1\/upload\?requestId=/u);
		expect(upload?.body?.get("file")).toBeInstanceOf(File);

		act(() => upload?.progress(25, 100));
		expect(
			await screen.findByLabelText("Model upload progress"),
		).toHaveAttribute("value", "0.25");
		expect(
			screen.getAllByText("Uploading Stage cube.glb — 25%").length,
		).toBeGreaterThan(0);

		act(() => upload?.progress(100, 100));
		expect(
			(await screen.findAllByText("Importing Stage cube.glb…")).length,
		).toBeGreaterThan(0);

		models.push(cube);
		act(() => upload?.respond(200, cube));
		expect(await screen.findByText("1/255 assigned")).toBeInTheDocument();
		expect(screen.queryByLabelText("Model upload progress")).toBeNull();
	});

	it("shows why the server refused a model", async () => {
		stubModels([]);
		const { container } = render(<ModelsPage />);
		await screen.findByText("0/255 assigned");

		chooseFile(container, "no-uvs.glb");
		act(() =>
			FakeUpload.last?.respond(422, {
				code: "model-missing-texture-coordinates",
				message:
					'mesh "Cube" has no texture coordinates (TEXCOORD_0); UV-unwrap it in your 3D tool and export again',
			}),
		);
		expect(
			await screen.findByText(
				/^no-uvs\.glb: mesh "Cube" has no texture coordinates/u,
			),
		).toHaveAttribute("role", "alert");
		expect(screen.getByText("0/255 assigned")).toBeInTheDocument();
	});

	it("says the server is unreachable instead of failing silently", async () => {
		stubModels([]);
		const { container } = render(<ModelsPage />);
		await screen.findByText("0/255 assigned");
		chooseFile(container, "cube.glb");
		act(() => FakeUpload.last?.onerror?.());
		expect(
			await screen.findByText(/^cube\.glb: the Media Server is not answering/u),
		).toHaveAttribute("role", "alert");
	});

	it("renames and clears a slot with intent edits", async () => {
		const user = userEvent.setup();
		const { writes } = stubModels([cube]);
		render(<ModelsPage />);
		await screen.findByText("1/255 assigned");

		const name = screen.getByLabelText("Name");
		await user.clear(name);
		await user.type(name, "Back wall");
		await user.click(screen.getByRole("button", { name: "Save name" }));
		await waitFor(() => expect(writes).toHaveLength(1));
		expect(writes[0].path).toBe("/api/v2/models/1/update");
		expect(writes[0].body).toMatchObject({ name: "Back wall" });
		expect(writes[0].body.requestId).toEqual(expect.any(String));

		await user.click(await screen.findByRole("button", { name: "Clear slot" }));
		await waitFor(() => expect(writes).toHaveLength(2));
		expect(writes[1].body).toMatchObject({ clear: true });
		expect(await screen.findByText("0/255 assigned")).toBeInTheDocument();
	});

	it("lists the built-in models a new server holds, Plane first", async () => {
		stubModels(BUILT_INS);
		const { container } = render(<ModelsPage />);
		expect(await screen.findByText("5/255 assigned")).toBeInTheDocument();
		const cards = [
			...container.querySelectorAll(".media-models-pool-grid .pool-card"),
		].slice(0, 6);
		expect(cards.map((card) => card.textContent)).toEqual([
			expect.stringContaining("Plane"),
			expect.stringContaining("Cube"),
			expect.stringContaining("Sphere"),
			expect.stringContaining("Cylinder"),
			expect.stringContaining("Pyramid"),
			expect.stringContaining("Empty"),
		]);
		expect(screen.getAllByText("Built-in")).toHaveLength(5);
		expect(screen.getByText(/^Built-in ·/u)).toHaveTextContent(
			"Built-in · 4 vertices · 2 triangles",
		);
		// Each card shows its model's picture rather than a plain colour.
		const pictures = cards
			.slice(0, 5)
			.map((card) => card.querySelector("img.pool-card-image")?.getAttribute("src"));
		expect(pictures[0]).toBe("/api/v2/models/1/preview?v=plane");
		expect(pictures[4]).toBe("/api/v2/models/5/preview?v=pyramid");
		expect(screen.getByRole("img", { name: "Picture of the Plane model" })).toBeInTheDocument();
	});

	it("keeps a built-in preset fixed and an imported model configurable", async () => {
		stubModels([...BUILT_INS, { ...cube, slot: 6 }]);
		const { container } = render(<ModelsPage />);
		await screen.findByText("6/255 assigned");
		// A preset has nothing to set: no file, no name, no other preset to switch to.
		expect(screen.getByText(/built-in preset: its shape is fixed/iu)).toBeInTheDocument();
		expect(screen.queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
		expect(screen.queryByText("Replace model")).not.toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Cube" })).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Clear slot" })).toBeInTheDocument();

		const cards = container.querySelectorAll(".media-models-pool-grid .pool-card");
		fireEvent.click(cards[5]);
		expect(await screen.findByRole("textbox", { name: "Name" })).toHaveValue("Stage cube");
		expect(screen.getByText("Replace model")).toBeInTheDocument();
		expect(screen.getByRole("img", { name: "Picture of the Stage cube model" })).toHaveAttribute(
			"src",
			"/api/v2/models/6/preview?v=24-12-Stage%20cube",
		);
	});

	it("falls back to the plain card when a model cannot be pictured", async () => {
		stubModels([{ ...cube, status: "unloadable", detail: "file missing" }]);
		const { container } = render(<ModelsPage />);
		await screen.findByText("1/255 assigned");
		const card = container.querySelector(".media-models-pool-grid .pool-card");
		expect(card?.querySelector("img.pool-card-image")).toBeNull();
		expect(card).toHaveTextContent("Cannot load");
		expect(screen.getByRole("status")).toHaveTextContent(
			"No picture: the model cannot be loaded",
		);
		// It can still be replaced.
		expect(screen.getByText("Replace model")).toBeInTheDocument();
	});

	it("assigns every built-in model to a slot without an upload", async () => {
		const user = userEvent.setup();
		const { writes } = stubModels([]);
		render(<ModelsPage />);
		await screen.findByText("0/255 assigned");
		for (const button of ["Plane", "Cube", "Sphere", "Cylinder", "Pyramid"]) {
			expect(screen.getByRole("button", { name: button })).toHaveAttribute(
				"aria-pressed",
				"false",
			);
		}

		const ids = ["plane", "cube", "sphere", "cylinder", "pyramid"];
		for (const [index, label] of [
			"Plane",
			"Cube",
			"Sphere",
			"Cylinder",
			"Pyramid",
		].entries()) {
			const slot = index + 1;
			fireEvent.click(
				document.querySelectorAll(".media-models-pool-grid .pool-card")[index],
			);
			await user.click(await screen.findByRole("button", { name: label }));
			await waitFor(() => expect(writes).toHaveLength(index + 1));
			expect(writes[index].path).toBe(`/api/v2/models/${slot}/update`);
			expect(writes[index].body).toMatchObject({ builtin: ids[index] });
			expect(writes[index].body.requestId).toEqual(expect.any(String));
			// Once placed, the preset is fixed: its inspector offers no other preset.
			await waitFor(() =>
				expect(screen.queryByRole("button", { name: label })).not.toBeInTheDocument(),
			);
		}
		expect(FakeUpload.last).toBeUndefined();
		expect(screen.getByText("5/255 assigned")).toBeInTheDocument();
	});
});
