import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import {
	MEDIA_SERVER_SECTIONS,
	MEDIA_SETTINGS_SECTIONS,
	MediaServerShell,
	MediaSettingsLayout,
} from "./MediaServerSurface";

describe("the Media Server operator surface", () => {
	it("shows only the six Media Server built-ins in their operator order", async () => {
		const navigate = vi.fn();
		render(
			<MediaServerShell
				active="dashboard"
				connected
				instance="Media Server 1"
				now={new Date("2026-08-12T20:42:00Z")}
				onNavigate={navigate}
			>
				<p>Current screen</p>
			</MediaServerShell>,
		);

		const dock = screen.getByRole("complementary", {
			name: "Media Server sections",
		});
		expect(
			within(dock).getByLabelText("ToskLight Media Server"),
		).toBeInTheDocument();
		expect(dock.querySelector("time")).toHaveAttribute(
			"datetime",
			"2026-08-12T20:42:00.000Z",
		);
		const destinations = within(dock).getByRole("navigation", {
			name: "Media Server destinations",
		});
		expect(
			within(destinations)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual(
			MEDIA_SERVER_SECTIONS.map((section) => `${section.icon}${section.label}`),
		);
		expect(
			within(dock).queryByText(/desktop|audio|dmx|logs/iu),
		).not.toBeInTheDocument();

		await userEvent.click(
			within(destinations).getByRole("button", { name: /Playback/ }),
		);
		expect(navigate).toHaveBeenCalledWith("media");
	});

	it("keeps the four settings areas in the Light Desk order", () => {
		render(
			<MediaSettingsLayout active="libraries">
				<p>Settings content</p>
			</MediaSettingsLayout>,
		);

		const settings = screen.getByRole("radiogroup", {
			name: "Media Server settings",
		});
		expect(
			within(settings)
				.getAllByRole("radio")
				.map((button) => button.textContent),
		).toEqual(MEDIA_SETTINGS_SECTIONS.map((section) => section.label));
	});

	it("identifies the connected Light Desk by its active show", () => {
		render(
			<MediaServerShell
				active="dashboard"
				connected
				instance="Media Server 1"
				showName="The Tempest"
				now={new Date("2026-08-12T20:42:00Z")}
				onNavigate={vi.fn()}
			>
				<p>Current screen</p>
			</MediaServerShell>,
		);

		expect(screen.getByText("Light Desk · The Tempest")).toBeInTheDocument();
	});
});
