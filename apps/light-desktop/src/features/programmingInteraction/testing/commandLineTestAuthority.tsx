import type { ReactNode } from "react";
import type { CommandTarget, ProgrammingSnapshot } from "../contracts";
import { ProgrammingInteractionViewProvider } from "../ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../store";

const DEFAULT_SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const DEFAULT_DESK_ID = "11111111-1111-4111-8111-111111111111";

export interface CommandLineTestWrite {
	deskId: string;
	text: string;
	expectedRevision: number;
}

interface CommandLineTestAuthorityOptions {
	text?: string;
	target?: CommandTarget;
	showId?: string;
	deskId?: string;
}

/** A real scoped command-line provider with an in-memory revisioned transport. */
export function createCommandLineTestAuthority({
	text,
	target = "FIXTURE",
	showId = DEFAULT_SHOW_ID,
	deskId = DEFAULT_DESK_ID,
}: CommandLineTestAuthorityOptions = {}) {
	const store = new ProgrammingInteractionStore();
	const writes: CommandLineTestWrite[] = [];
	let projection = commandProjection(1, text ?? target, target);
	const loadSnapshot = async (): Promise<ProgrammingSnapshot> => ({
		cursor: 1,
		projection: {
			deskId,
			commandLine: projection,
			selection: {
				selected: [],
				expression: { type: "static" },
				revision: 1,
				gestureOpen: false,
			},
		},
	});
	const replaceCommandLine = async (
		requestedDeskId: string,
		value: string,
		expectedRevision: number,
	) => {
		writes.push({
			deskId: requestedDeskId,
			text: value,
			expectedRevision,
		});
		projection = commandProjection(
			expectedRevision + 1,
			value.trim() ? value : target,
			target,
		);
		return projection;
	};
	const wrap = (children: ReactNode) => (
		<ProgrammingInteractionViewProvider
			showId={showId}
			deskId={deskId}
			store={store}
			transport={null}
			loadSnapshot={loadSnapshot}
			replaceCommandLine={replaceCommandLine}
		>
			{children}
		</ProgrammingInteractionViewProvider>
	);
	const settle = async () => {
		await Promise.resolve();
		await Promise.resolve();
		await Promise.resolve();
	};
	return { deskId, store, writes, wrap, settle };
}

function commandProjection(
	revision: number,
	text: string,
	target: CommandTarget,
) {
	return {
		text,
		target,
		pristine: text.trim().toUpperCase() === target,
		revision,
		pendingChoice: null,
	};
}
