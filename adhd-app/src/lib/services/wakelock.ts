let wakeLock: WakeLockSentinel | null = null;

export async function requestWakeLock(): Promise<boolean> {
	try {
		if ('wakeLock' in navigator) {
			wakeLock = await navigator.wakeLock.request('screen');
			wakeLock.addEventListener('release', () => { wakeLock = null; });
			return true;
		}
	} catch {
		// Wake lock request failed (e.g. low battery)
	}
	return false;
}

export function releaseWakeLock(): void {
	wakeLock?.release();
	wakeLock = null;
}

export function isWakeLockActive(): boolean {
	return wakeLock !== null;
}
