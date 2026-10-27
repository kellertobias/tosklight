/** Current-window editors register saves that must finish before replacing or copying the file. */
const pendingSaves = new Set<() => Promise<void>>();

export function registerPendingDocumentSave(flush: () => Promise<void>): () => void {
	pendingSaves.add(flush);
	return () => { pendingSaves.delete(flush); };
}

/** Failed saves reject the file action; the editor retains its draft and existing Retry action. */
export async function afterPendingDocumentSaves<T>(action: () => Promise<T>): Promise<T> {
	for (const flush of pendingSaves) await flush();
	return action();
}
