/** The real conversion panel with an offline snapshot, never a running FFmpeg job. */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { WindowFrame } from "@tosklight/ui/window-kit";
import { ImportPanel } from "../features/media-library/ImportPanel";
import { api } from "../shared/api/client";
import type { ImportsView } from "../shared/api/generated/media-wire";
import "../styles.css";

const imports: ImportsView = {
	canImport: true,
	pending: [],
	jobs: [
		{
			id: "converted",
			batchId: "opening-night",
			address: { folder: 1, file: 1, class: "library" },
			filename: "Opening Night.mp4",
			state: "succeeded",
			attempts: 1,
			fraction: 1,
			framesDone: 1800,
			framesTotal: 1800,
			reason: null,
		},
		{
			id: "converting",
			batchId: "opening-night",
			address: { folder: 1, file: 2, class: "library" },
			filename: "Interval Loop.mov",
			state: "running",
			attempts: 1,
			fraction: 0.68,
			framesDone: 1224,
			framesTotal: 1800,
			reason: null,
		},
		{
			id: "queued",
			batchId: "opening-night",
			address: { folder: 1, file: 3, class: "library" },
			filename: "Finale.mov",
			state: "queued",
			attempts: 0,
			fraction: null,
			framesDone: null,
			framesTotal: null,
			reason: null,
		},
	],
};
function installOfflineImports() {
	const originalImports = api.imports;
	const socketDescriptor = Object.getOwnPropertyDescriptor(
		globalThis,
		"WebSocket",
	);
	api.imports = async () => imports;
	// ImportPanel keeps its production snapshot path, with no socket connection in this offline story.
	Object.defineProperty(globalThis, "WebSocket", {
		configurable: true,
		value: undefined,
	});
	return () => {
		api.imports = originalImports;
		if (socketDescriptor)
			Object.defineProperty(globalThis, "WebSocket", socketDescriptor);
		else Reflect.deleteProperty(globalThis, "WebSocket");
	};
}
const meta = {
	title: "ToskLight/Media Server/Import Progress",
	parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
export const Converting: Story = {
	beforeEach: installOfflineImports,
	render: () => (
		<div style={{ width: 1000, padding: 18 }}>
			<WindowFrame title="Library">
				<ImportPanel onImported={() => undefined} />
			</WindowFrame>
		</div>
	),
};
