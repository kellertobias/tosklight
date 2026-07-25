import type { ProgrammingChange, ProgrammingCapability } from "./contracts";
import type { ProgrammingEventScope } from "./transport";

export type ProgrammingViewCapabilities = ProgrammingEventScope;

export class ProgrammingViewScope {
	private readonly references: Record<ProgrammingCapability, number> = {
		commandLine: 0,
		selection: 0,
	};

	activate(capability: ProgrammingCapability) {
		const previous = this.references[capability];
		this.references[capability]++;
		return previous === 0;
	}

	deactivate(capability: ProgrammingCapability) {
		const previous = this.references[capability];
		this.references[capability] = Math.max(0, previous - 1);
		return previous === 1;
	}

	hasViews() {
		return this.references.commandLine > 0 || this.references.selection > 0;
	}

	includesChange(change: ProgrammingChange) {
		return (
			("commandLine" in change && this.references.commandLine > 0) ||
			("selection" in change && this.references.selection > 0)
		);
	}

	subscription(): ProgrammingEventScope {
		return {
			commandLine: this.references.commandLine > 0,
			selection: this.references.selection > 0,
		};
	}

	key() {
		return JSON.stringify(this.subscription());
	}

	clear() {
		this.references.commandLine = 0;
		this.references.selection = 0;
	}
}
