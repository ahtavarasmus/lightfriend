import { openDB, type IDBPDatabase } from 'idb';
import type { UserSettings } from './state';

const DB_NAME = 'iltavahti-db';
const DB_VERSION = 1;

let dbPromise: Promise<IDBPDatabase> | null = null;

function getDB(): Promise<IDBPDatabase> {
	if (!dbPromise) {
		dbPromise = openDB(DB_NAME, DB_VERSION, {
			upgrade(db) {
				if (!db.objectStoreNames.contains('settings')) {
					db.createObjectStore('settings');
				}
				if (!db.objectStoreNames.contains('sleepLog')) {
					db.createObjectStore('sleepLog', { keyPath: 'date' });
				}
				if (!db.objectStoreNames.contains('interactions')) {
					db.createObjectStore('interactions', { keyPath: 'id', autoIncrement: true });
				}
			}
		});
	}
	return dbPromise;
}

export async function saveSettings(s: UserSettings): Promise<void> {
	const db = await getDB();
	await db.put('settings', s, 'current');
}

export async function loadSettings(): Promise<UserSettings | undefined> {
	const db = await getDB();
	return db.get('settings', 'current');
}

export interface InteractionRecord {
	id?: number;
	type: string;
	timestamp: number;
	durationMs: number;
	completed: boolean;
}

export async function logInteraction(record: Omit<InteractionRecord, 'id'>): Promise<void> {
	const db = await getDB();
	await db.add('interactions', record);
}

export async function getRecentInteractions(limit = 10): Promise<InteractionRecord[]> {
	const db = await getDB();
	const all = await db.getAll('interactions');
	return all.slice(-limit);
}

export interface SleepLogEntry {
	date: string;
	bedtime: string;
	wakeTime: string;
	sleepHours: number;
	intensityReached: string;
}

export async function logSleep(entry: SleepLogEntry): Promise<void> {
	const db = await getDB();
	await db.put('sleepLog', entry);
}
