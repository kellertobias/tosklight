import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DeskIdentityProvider } from "../../operator/DeskIdentityContext";
import { stubServer } from "../../testing/server";
import { DashboardPage } from "./DashboardPage";

afterEach(() => vi.unstubAllGlobals());

describe("the dashboard", () => {
	it("renders the live dashboard content inside its padded surface", async () => {
		stubServer();
		const { container } = render(
			<DeskIdentityProvider showName="The Tempest">
				<DashboardPage />
			</DeskIdentityProvider>,
		);

		expect(container.querySelector(".media-dashboard-page")).toBeInTheDocument();
		expect(await screen.findByText("The Tempest")).toBeInTheDocument();
		expect(await screen.findByRole("heading", { name: "Main" })).toBeVisible();
	});
});
