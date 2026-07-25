type Method = (...args: never[]) => unknown;

type MethodName<T> = {
	[K in keyof T]-?: T[K] extends Method ? K : never;
}[keyof T];

/** Keeps the flat public facade explicit while capability clients retain ownership. */
export function bindClientMethod<T, K extends MethodName<T>>(
	client: T,
	methodName: K,
): T[K] {
	const method = client[methodName] as Method;
	return method.bind(client) as T[K];
}
