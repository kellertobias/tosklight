import type { Meta, StoryObj } from "@storybook/react-vite";
import { type PropsWithChildren, useMemo, useState } from "react";
import type { CueList, PlaybackDefinition } from "../../api/types";
import { ShowObjectsStateProvider } from "../../features/showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../../features/showObjects/store";
import { PlaybackConfigurationDialog } from "../control/PlaybackConfigurationModal";
import { type RecordMode, RecordModeDialog } from "../shared/RecordModeDialog";

const meta = {
	title: "ToskLight/Modal workflows",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const cueList: CueList = {
	id: "main-sequence",
	name: "Main Sequence",
	mode: "sequence",
	priority: 10,
	looped: false,
	cues: [],
};

const playback: PlaybackDefinition = {
	number: 1,
	name: "Main Sequence",
	target: { type: "cue_list", cue_list_id: cueList.id },
	buttons: ["go_minus", "go", "flash"],
	button_count: 3,
	fader: "master",
	has_fader: true,
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
	color: "#20c997",
};

function ShowObjectsHarness({ children }: PropsWithChildren) {
	const store = useMemo(() => {
		const next = new ShowObjectsStore();
		next.reset("storybook-modal-workflows");
		next.setCollection("storybook-modal-workflows", "group", []);
		next.setCollection("storybook-modal-workflows", "cue_list", [
			{
				kind: "cue_list",
				id: "main-sequence-object",
				body: cueList,
				revision: 3,
				updated_at: "2026-07-26T12:00:00Z",
			},
		]);
		return next;
	}, []);
	return (
		<ShowObjectsStateProvider store={store}>
			{children}
		</ShowObjectsStateProvider>
	);
}

function PlaybackConfigurationStory() {
	const [result, setResult] = useState("No mutation");
	return (
		<ShowObjectsHarness>
			<div className="app-shell">
				<output aria-label="Last playback configuration mutation">
					{result}
				</output>
				<PlaybackConfigurationDialog
					playback={playback}
					page={1}
					slot={1}
					fallbackButtons={3}
					save={async (_page, _slot, next) => {
						setResult(`Saved ${next.name}`);
						return true;
					}}
					clear={async () => {
						setResult("Cleared playback 1.1");
						return true;
					}}
					onClose={() => setResult("Closed without saving")}
				/>
			</div>
		</ShowObjectsHarness>
	);
}

export const PlaybackConfiguration: Story = {
	render: () => <PlaybackConfigurationStory />,
};

function RecordModeStory() {
	const [choice, setChoice] = useState<RecordMode | "waiting">("waiting");
	return (
		<div className="app-shell">
			<output aria-label="Record mode choice">{choice}</output>
			<RecordModeDialog
				target="Cuelist 1"
				onCancel={() => setChoice("waiting")}
				onChoose={setChoice}
			/>
		</div>
	);
}

export const RecordExistingTarget: Story = {
	render: () => <RecordModeStory />,
};
