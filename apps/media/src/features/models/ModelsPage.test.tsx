import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
	status: "ready",
	detail: null,
};

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
				models[index] = { ...models[index], name: String(body.name) };
				return Response.json(models[index]);
			}
			return Response.json({ code: "not-found", message: path }, { status: 404 });
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
		expect(screen.getAllByText("Uploading Stage cube.glb — 25%").length).toBeGreaterThan(0);

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
});
