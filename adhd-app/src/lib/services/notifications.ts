import type { IntensityLevel } from '$lib/core/events';

const messages: Record<IntensityLevel, string[]> = {
	calm: ['Hyvää iltaa! Muista levätä ajoissa.'],
	gentle: ['Ilta etenee. Ala rauhoittumaan.', 'Hei, kello käy. Kohta nukkumaan.'],
	warning: ['Nukkumaanmenoaika lähestyy!', 'Unta jää vähemmän joka minuutti.'],
	urgent: ['MENE NUKKUMAAN NYT!', 'Huominen tulee, halusit tai et.'],
	overdue: ['Olet jo myöhässä. Jokainen minuutti maksaa.', 'Aamu tulee silti. Mene sänkyyn.']
};

let permissionGranted = false;

export async function requestPermission(): Promise<boolean> {
	if (typeof Notification === 'undefined') return false;
	const result = await Notification.requestPermission();
	permissionGranted = result === 'granted';
	return permissionGranted;
}

export function sendNotification(level: IntensityLevel): void {
	if (!permissionGranted) return;
	const pool = messages[level];
	const msg = pool[Math.floor(Math.random() * pool.length)];
	new Notification('Iltavahti', { body: msg, tag: 'iltavahti', renotify: true });
}
