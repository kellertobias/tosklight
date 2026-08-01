import "@testing-library/jest-dom/vitest";
import { ModalProvider } from "@tosklight/ui/modals";
import { createElement, type PropsWithChildren } from "react";
import { vi } from "vitest";

// The patch dialogs render through the shared modal host, exactly as they do in a product shell.
vi.mock("@testing-library/react", async (importOriginal) => {
	const testing =
		await importOriginal<typeof import("@testing-library/react")>();
	return {
		...testing,
		render(
			ui: Parameters<typeof testing.render>[0],
			options?: Parameters<typeof testing.render>[1],
		) {
			const ExistingWrapper = options?.wrapper;
			function ProductionRoot({ children }: PropsWithChildren) {
				const content = ExistingWrapper
					? createElement(ExistingWrapper, null, children)
					: children;
				return createElement(ModalProvider, null, content);
			}
			return testing.render(ui, { ...options, wrapper: ProductionRoot });
		},
	};
});
