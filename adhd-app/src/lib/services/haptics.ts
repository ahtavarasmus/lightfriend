import type { IntensityLevel } from '$lib/core/events';

function vibrate(pattern: number[]): void {
	if ('vibrate' in navigator) {
		navigator.vibrate(pattern);
	}
}

export function vibrateForLevel(level: IntensityLevel): void {
	switch (level) {
		case 'gentle':
			vibrate([100]);
			break;
		case 'warning':
			vibrate([200, 100, 200]);
			break;
		case 'urgent':
			vibrate([300, 100, 300, 100, 300]);
			break;
		case 'overdue':
			vibrate([500, 200, 500, 200, 500, 200, 500]);
			break;
	}
}
