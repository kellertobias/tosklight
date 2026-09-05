import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ToastProvider, useFailureToast } from "../../app/ToastContext";
import { newId } from "../../features/pixelmap/pixelMapEditing";
import { requestId } from "./editing";
import { newIdentity } from "./identity";

afterEach(() => vi.unstubAllGlobals());

function lanCrypto(): void {
	const getRandomValues = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
	vi.stubGlobal("crypto", { getRandomValues });
}

describe("LAN HTTP identities", () => {
	it("creates unique valid request UUIDs without secure-context randomUUID", () => {
		lanCrypto();
		const ids = Array.from({ length: 100 }, () => requestId());
		expect(new Set(ids).size).toBe(ids.length);
		for (const id of ids)
			expect(id).toMatch(
				/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
			);
	});

	it("does not give newly created pixel zones the same timestamp prefix", () => {
		lanCrypto();
		const ids = Array.from({ length: 100 }, () => newId("zone"));
		expect(new Set(ids).size).toBe(ids.length);
	});

	it("keeps the native UUID path when it is available", () => {
		const randomUUID = vi.fn(() => "native-uuid");
		vi.stubGlobal("crypto", { randomUUID });
		expect(newIdentity()).toBe("native-uuid");
		expect(randomUUID).toHaveBeenCalledOnce();
	});

	it("can report and dismiss a failure over LAN HTTP", () => {
		lanCrypto();
		function FailureButton() {
			useFailureToast({ message: "Upload failed" });
			return null;
		}
		render(
			<ToastProvider><FailureButton /></ToastProvider>,
		);
		expect(screen.getByRole("alert")).toHaveTextContent("Upload failed");
		fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
	});
});
