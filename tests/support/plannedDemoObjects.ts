import type { ApiDriver } from "../bench/core/api";

export async function putPlannedDemoObject(
	api: ApiDriver,
	showId: string,
	kind: string,
	id: string,
	body: unknown,
) {
	let revision = 0;
	if (kind === "preset") {
		const current = await api.showObject<any>(showId, kind, id);
		if (current) {
			revision = current.revision;
			body = {
				...(body as Record<string, unknown>),
				instance_id: current.body.instance_id,
			};
		}
	}
	for (let attempt = 0; attempt < 4; attempt++) {
		try {
			await api.seedShowObject(showId, kind, id, body, revision);
			return;
		} catch (error) {
			if (
				!(error instanceof Error) ||
				!error.message.includes("revision conflict")
			)
				throw error;
			const current = await api.showObject(showId, kind, id);
			if (!current) throw error;
			revision = current.revision;
		}
	}
	throw new Error(
		`Could not write demo ${kind} ${id} after repeated revision conflicts`,
	);
}
