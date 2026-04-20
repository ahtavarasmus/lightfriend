type EventMap = {
	'intensity-changed': { level: IntensityLevel; hoursRemaining: number };
	'interaction-required': { type: string; deadline: number };
	'interaction-completed': { type: string; durationMs: number };
	'interaction-timeout': { type: string };
	'bedtime-reached': void;
	'settings-updated': void;
};

export type IntensityLevel = 'calm' | 'gentle' | 'warning' | 'urgent' | 'overdue';

type Handler<T> = (data: T) => void;

const listeners = new Map<string, Set<Handler<unknown>>>();

export function on<K extends keyof EventMap>(event: K, handler: Handler<EventMap[K]>): () => void {
	if (!listeners.has(event)) listeners.set(event, new Set());
	const set = listeners.get(event)!;
	set.add(handler as Handler<unknown>);
	return () => set.delete(handler as Handler<unknown>);
}

export function emit<K extends keyof EventMap>(event: K, data: EventMap[K]): void {
	listeners.get(event)?.forEach((h) => h(data));
}

export function off<K extends keyof EventMap>(event: K, handler: Handler<EventMap[K]>): void {
	listeners.get(event)?.delete(handler as Handler<unknown>);
}
