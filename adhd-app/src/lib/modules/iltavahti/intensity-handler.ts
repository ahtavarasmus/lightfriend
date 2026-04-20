import type { IntensityLevel } from '$lib/core/events';
import { emit } from '$lib/core/events';
import type { IntensityPreference } from '$lib/core/state';
import { sendNotification } from '$lib/services/notifications';
import { vibrateForLevel } from '$lib/services/haptics';
import { requestWakeLock, releaseWakeLock } from '$lib/services/wakelock';
import { pickInteraction } from './interaction-picker';

let lastLevel: IntensityLevel = 'calm';
let interactionInterval: ReturnType<typeof setInterval> | null = null;

function shouldAct(level: IntensityLevel, pref: IntensityPreference): boolean {
	if (level === 'calm') return false;
	if (pref === 'light' && (level === 'gentle')) return false;
	return true;
}

function getInteractionIntervalMs(level: IntensityLevel): number {
	switch (level) {
		case 'warning': return 5 * 60 * 1000;  // 5 min
		case 'urgent': return 2 * 60 * 1000;    // 2 min
		case 'overdue': return 60 * 1000;        // 1 min
		default: return 0;
	}
}

async function triggerInteraction(): Promise<void> {
	const interaction = await pickInteraction();
	emit('interaction-required', {
		type: interaction.type,
		deadline: Date.now() + interaction.timeLimit * 1000
	});
}

export function handleIntensityChange(
	level: IntensityLevel,
	hoursRemaining: number,
	pref: IntensityPreference
): void {
	if (level === lastLevel) return;
	lastLevel = level;

	emit('intensity-changed', { level, hoursRemaining });

	if (!shouldAct(level, pref)) return;

	sendNotification(level);
	vibrateForLevel(level);

	if (interactionInterval) {
		clearInterval(interactionInterval);
		interactionInterval = null;
	}

	if (level === 'urgent' || level === 'overdue') {
		requestWakeLock();
		triggerInteraction();
		const ms = getInteractionIntervalMs(level);
		if (ms > 0) {
			interactionInterval = setInterval(triggerInteraction, ms);
		}
	} else if (level === 'warning' && pref !== 'light') {
		triggerInteraction();
		const ms = getInteractionIntervalMs(level);
		if (ms > 0) {
			interactionInterval = setInterval(triggerInteraction, ms);
		}
	} else {
		releaseWakeLock();
	}
}

export function stopIntensityHandler(): void {
	if (interactionInterval) {
		clearInterval(interactionInterval);
		interactionInterval = null;
	}
	releaseWakeLock();
	lastLevel = 'calm';
}
