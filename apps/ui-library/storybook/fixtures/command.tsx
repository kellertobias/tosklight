import { CommandLine, type CommandLineMode } from "../../src/command";

export function StaticCommandLine({
	mode,
	hardware,
	commandLine,
	preloadArmed = false,
	onToggleMode = () => undefined,
}: {
	mode: CommandLineMode;
	hardware: boolean;
	commandLine: string;
	preloadArmed?: boolean;
	onToggleMode?: () => void;
}) {
	return (
		<CommandLine
			mode={mode}
			hardware={hardware}
			ready
			completed={false}
			commandError={null}
			persistentError={null}
			persistentErrorOpen={false}
			commandLine={commandLine}
			commandTarget="FIXTURE"
			preloadArmed={preloadArmed}
			preloadActive={preloadArmed}
			preloadReady
			preloadLabel={preloadArmed ? "PRELOAD GO" : "PRELOAD"}
			pendingSummary={preloadArmed ? "PROG 4 · GO MINUS 2" : ""}
			recordState="ready"
			recordShiftArmed={false}
			history={[]}
			historyOpen={false}
			status={{
				connection: "connected",
				frequency: 44,
				timecode: null,
				blackout: false,
				highlight: false,
			}}
			onReplace={() => undefined}
			onExecute={() => undefined}
			onToggleMode={onToggleMode}
			onHistoryOpenChange={() => undefined}
			onReuseHistory={() => undefined}
			onOpenStatus={() => undefined}
			onAcknowledgeCommandError={() => undefined}
			onPersistentErrorOpenChange={() => undefined}
			onAcknowledgePersistentError={() => undefined}
			onRecordStart={() => undefined}
			onRecordEnd={() => undefined}
			onRecordCancel={() => undefined}
			onRecordComplete={() => undefined}
			onAdvancePreload={() => undefined}
			onInspectPreload={() => undefined}
		/>
	);
}
