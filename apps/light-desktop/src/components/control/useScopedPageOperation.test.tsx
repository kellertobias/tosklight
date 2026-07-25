import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { startTransition, StrictMode, Suspense, useEffect, useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { useScopedPageOperation } from "./useScopedPageOperation";

type PageOperation = ReturnType<typeof useScopedPageOperation>;

let committed: PageOperation | null = null;
let suspendedRenders = 0;
const never = new Promise<never>(() => undefined);

function Probe({ scope, suspend = false }: { scope: string; suspend?: boolean }) {
	const operation = useScopedPageOperation([scope]);
	if (suspend) {
		suspendedRenders += 1;
		throw never;
	}
	committed = operation;
	return <span>{operation.failure ?? operation.pending ?? "idle"}</span>;
}

function StrictModeMenuProbe() {
	const operation = useScopedPageOperation(["show-a"]);
	const [open, setOpen] = useState(false);
	useEffect(() => setOpen(false), [operation.generation]);
	return (
		<>
			<button type="button" onClick={() => setOpen(true)}>
				Open
			</button>
			<span>{open ? "menu open" : "menu closed"}</span>
		</>
	);
}

afterEach(() => {
	cleanup();
	committed = null;
	suspendedRenders = 0;
});

describe("useScopedPageOperation", () => {
	it("does not invalidate a committed operation from an abandoned render", async () => {
		const view = render(
			<Suspense fallback={<span>replacement loading</span>}>
				<Probe scope="show-a" />
			</Suspense>,
		);
		let token: number | null = null;
		act(() => {
			token = committed?.begin("select") ?? null;
		});
		expect(screen.getByText("select")).toBeInTheDocument();

		act(() => {
			startTransition(() => {
				view.rerender(
					<Suspense fallback={<span>replacement loading</span>}>
						<Probe scope="show-b" suspend />
					</Suspense>,
				);
			});
		});
		await waitFor(() => expect(suspendedRenders).toBeGreaterThan(0));
		expect(screen.queryByText("replacement loading")).toBeNull();

		let accepted = false;
		act(() => {
			accepted = committed?.complete(token!, "selection failed") ?? false;
		});
		expect(accepted).toBe(true);
		expect(screen.getByText("selection failed")).toBeInTheDocument();
	});

	it("exposes idle state immediately when replacement authority commits", () => {
		const view = render(<Probe scope="show-a" />);
		act(() => {
			committed?.begin("rename");
		});
		expect(screen.getByText("rename")).toBeInTheDocument();

		view.rerender(<Probe scope="show-b" />);

		expect(screen.getByText("idle")).toBeInTheDocument();
		expect(committed?.busy).toBe(false);
	});

	it("does not change generation during the StrictMode mount replay", () => {
		render(
			<StrictMode>
				<StrictModeMenuProbe />
			</StrictMode>,
		);

		act(() => screen.getByRole("button", { name: "Open" }).click());

		expect(screen.getByText("menu open")).toBeInTheDocument();
	});

	it("rejects completion after the owner unmounts", () => {
		const view = render(<Probe scope="show-a" />);
		let token: number | null = null;
		act(() => {
			token = committed?.begin("rename") ?? null;
		});
		const operation = committed;

		view.unmount();

		expect(operation?.isCurrent(token!)).toBe(false);
		expect(operation?.complete(token!, "late failure")).toBe(false);
	});
});
