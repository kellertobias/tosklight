import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SwitchField } from "@tosklight/ui/controls";
import { describe, expect, it, vi } from "vitest";
import {
	MEDIA_SERVER_SECTIONS,
	MEDIA_SETTINGS_SECTIONS,
	MediaServerShell,
	MediaSettingsLayout,
} from "./MediaServerSurface";

describe("the Media Server operator surface", () => {
	it("shows the Media Server destinations in their operator order", async () => {
		const navigate = vi.fn();
		render(
			<MediaServerShell
				active="media"
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
		expect(dock.querySelector(".media-operator-mark img")).toHaveAttribute(
			"src",
			expect.stringContaining("tosklight-media-icon.svg"),
		);
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
		expect(within(dock).getByText("DMX")).toBeInTheDocument();
		expect(within(dock).queryByText("Dashboard")).not.toBeInTheDocument();
		expect(
			within(dock).queryByText(/desktop|logs|visualizers|text/iu),
		).not.toBeInTheDocument();

		await userEvent.click(
			within(destinations).getByRole("button", { name: /Playback/ }),
		);
		expect(navigate).toHaveBeenCalledWith("media");

		await userEvent.click(
			within(destinations).getByRole("button", { name: /Audio/ }),
		);
		expect(navigate).toHaveBeenCalledWith("audio");
	});

	it("keeps the dedicated settings screens in the window title", () => {
		render(
			<MediaSettingsLayout active="libraries">
				<p>Settings content</p>
			</MediaSettingsLayout>,
		);

		const settings = screen.getByRole("tablist");
		expect(
			within(settings)
				.getAllByRole("tab")
				.map((button) => button.textContent),
		).toEqual(MEDIA_SETTINGS_SECTIONS.map((section) => section.label));
	});

	it("identifies the connected Light Desk by its active show", () => {
		render(
			<MediaServerShell
				active="media"
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

	it("shows one bare playback takeover control on every operator screen", () => {
		for (const section of MEDIA_SERVER_SECTIONS) {
			const { unmount } = render(
				<MediaServerShell
					active={section.id}
					connected
					now={new Date("2026-08-12T20:42:00Z")}
					playbackOwnership={
						<SwitchField bare label="Take over playback" checked={false} />
					}
				>
					<p>{section.label} window</p>
				</MediaServerShell>,
			);

			const dock = screen.getByRole("complementary", {
				name: "Media Server sections",
			});
			const ownership = within(dock).getByLabelText("Playback ownership");
			expect(within(ownership).getByRole("switch")).toHaveAccessibleName(
				"Take over playback",
			);
			expect(ownership.querySelector(".ui-switch-field-bare")).not.toBeNull();
			expect(ownership.querySelector(".ui-form-control")).toBeNull();
			unmount();
		}
	});
});
