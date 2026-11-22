import type { Meta, StoryObj } from "@storybook/react-vite";
import { MediaServerShell } from "../../operator/MediaServerSurface";
import { PlaylistsPage } from "./PlaylistsPage";
import { SchedulePage } from "./SchedulePage";
import { installAutomationExample } from "./automationStoryFixture";
import "../../styles.css";
import "../../operator/mediaServerSurface.css";

/** Production pages, with an isolated offline example for repeatable documentation captures. */
const meta = {
	title: "ToskLight/Media Server/Automation",
	parameters: { layout: "fullscreen" },
	beforeEach: installAutomationExample,
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
function Example({ page }: { page: "playlists" | "schedule" }) {
	return (
		<div className="marketing-screenshot-viewport">
			<MediaServerShell
				active={page}
				connected
				instance="Pixel · Lobby"
				now={new Date("2026-11-23T11:30:00+01:00")}
				onNavigate={(section) => {
					const automation = section === "playlists" || section === "schedule";
					const story =
						section === "settings"
							? "settings-libraries"
							: section === "dmx"
								? "dmx-diagnostics"
								: section;
					(window.top ?? window).location.assign(
						`/?path=/story/tosklight-media-server${automation ? "-automation" : ""}--${story}`,
					);
				}}
			>
				{page === "playlists" ? <PlaylistsPage /> : <SchedulePage />}
			</MediaServerShell>
		</div>
	);
}
export const Playlists: Story = { render: () => <Example page="playlists" /> };
export const Schedule: Story = { render: () => <Example page="schedule" /> };
