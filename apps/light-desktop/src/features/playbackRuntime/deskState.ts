import type { PlaybackDesk } from "./contracts";

interface PageOperation {
	token: string;
	baseSequence: number;
	order: number;
	page: number;
	surfaceId: string;
}

export class PlaybackDeskState {
	private authoritative: PlaybackDesk | null = null;
	private readonly operations = new Map<string, PageOperation>();
	private latestToken: string | null = null;
	private sequence = 0;
	private order = 0;

	reset() {
		this.authoritative = null;
		this.operations.clear();
		this.latestToken = null;
		this.sequence = 0;
		this.order = 0;
	}

	captureSequence() {
		return this.sequence;
	}

	hydrate(projection: PlaybackDesk, sequence: number) {
		if (sequence < this.sequence) return null;
		this.install(projection, sequence);
		return this.render();
	}

	apply(projection: PlaybackDesk, sequence: number) {
		if (sequence <= this.sequence) return null;
		this.install(projection, sequence);
		return this.render();
	}

	installUnsequenced(projection: PlaybackDesk, baseSequence: number) {
		if (this.sequence > baseSequence) return this.render();
		this.authoritative = projection;
		this.reconcile(projection);
		return this.render();
	}

	begin(page: number) {
		if (!this.authoritative) return null;
		const operation = {
			token: crypto.randomUUID(),
			baseSequence: this.sequence,
			order: ++this.order,
			page,
			surfaceId: this.authoritative.selected_surface_id,
		};
		this.operations.set(operation.token, operation);
		this.latestToken = operation.token;
		return { token: operation.token, desk: this.render() };
	}

	commit(token: string | null, sequence: number | null) {
		const operation = this.take(token);
		if (!operation) return null;
		this.discardThrough(operation.order);
		if (
			this.authoritative &&
			(sequence != null
				? sequence >= this.sequence
				: this.sequence <= operation.baseSequence)
		) {
			this.authoritative = {
				...this.authoritative,
				command_page:
					this.authoritative.selected_surface_id === operation.surfaceId
						? operation.page
						: this.authoritative.command_page,
				resolved_pages: {
					...this.authoritative.resolved_pages,
					[operation.surfaceId]: operation.page,
				},
			};
			if (sequence != null) this.sequence = Math.max(this.sequence, sequence);
		}
		return this.render();
	}

	rollback(token: string | null) {
		return this.take(token) ? this.render() : null;
	}

	private install(projection: PlaybackDesk, sequence: number) {
		this.authoritative = projection;
		this.sequence = sequence;
		this.reconcile(projection);
	}

	private reconcile(projection: PlaybackDesk) {
		for (const operation of this.operations.values())
			if (projection.resolved_pages[operation.surfaceId] === operation.page)
				this.operations.delete(operation.token);
		this.selectLatest();
	}

	private discardThrough(order: number) {
		for (const operation of this.operations.values())
			if (operation.order <= order) this.operations.delete(operation.token);
		this.selectLatest();
	}

	private take(token: string | null) {
		if (!token) return null;
		const operation = this.operations.get(token);
		if (!operation) return null;
		this.operations.delete(token);
		this.selectLatest();
		return operation;
	}

	private selectLatest() {
		this.latestToken =
			[...this.operations.values()].sort(
				(left, right) => right.order - left.order,
			)[0]?.token ?? null;
	}

	private render() {
		if (!this.authoritative) return null;
		if (!this.operations.size) return this.authoritative;
		const resolved_pages = { ...this.authoritative.resolved_pages };
		for (const operation of [...this.operations.values()].sort(
			(left, right) => left.order - right.order,
		))
			resolved_pages[operation.surfaceId] = operation.page;
		return {
			...this.authoritative,
			resolved_pages,
			command_page:
				resolved_pages[this.authoritative.selected_surface_id] ??
				this.authoritative.command_page,
		};
	}
}
