import type { ScheduleRuntimeChange } from "../../api/client/schedules";

export class SchedulerRuntimeStore {
	private listeners = new Set<() => void>();
	private revision = 0;
	private change: ScheduleRuntimeChange | null = null;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	snapshot = () => ({ revision: this.revision, change: this.change });

	install(change: ScheduleRuntimeChange) {
		this.change = change;
		this.revision += 1;
		for (const listener of this.listeners) listener();
	}
}
