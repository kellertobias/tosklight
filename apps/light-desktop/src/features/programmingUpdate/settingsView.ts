import type { UpdateSettings } from "../../api/types";

/** One settings read shared by the visible grids of one scoped desk writer. */
class ProgrammingUpdateSettingsView {
	private snapshot: UpdateSettings | null = null;
	private listeners = new Set<() => void>();
	private pending: Promise<void> | null = null;
	getSnapshot = () => this.snapshot;
	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	install(settings: UpdateSettings) {
		this.snapshot = settings;
		for (const listener of this.listeners) listener();
	}

	ensure(load: () => Promise<UpdateSettings | null>) {
		if (this.pending) return this.pending;
		this.pending = Promise.resolve()
			.then(load)
			.then(settings => {
				if (settings) this.install(settings);
			})
			.catch(() => undefined)
			.finally(() => {
				this.pending = null;
			});
		return this.pending;
	}
}

const views = new WeakMap<object, ProgrammingUpdateSettingsView>();
export function programmingUpdateSettingsView(source: object) {
	let view = views.get(source);
	if (!view) {
		view = new ProgrammingUpdateSettingsView();
		views.set(source, view);
	}
	return view;
}
