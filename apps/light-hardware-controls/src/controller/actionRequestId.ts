let fallbackSequence = 0;

export function actionRequestId(): string {
  return (
    globalThis.crypto?.randomUUID?.() ??
    `hardware-${Date.now()}-${++fallbackSequence}`
  );
}
