<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { settings } from '$lib/core/state';
	import { on, type IntensityLevel } from '$lib/core/events';
	import { requestPermission } from '$lib/services/notifications';
	import {
		sleepIfNow,
		calculateDeadline,
		getIntensityLevel,
		getIntensityColor,
		hoursUntilDeadline,
		formatHoursMinutes,
		formatTime
	} from '$lib/modules/iltavahti/time-engine';
	import { handleIntensityChange, stopIntensityHandler } from '$lib/modules/iltavahti/intensity-handler';
	import { pickInteraction, type Interaction } from '$lib/modules/iltavahti/interaction-picker';
	import InteractionModal from '$lib/modules/iltavahti/InteractionModal.svelte';
	import IntensityOverlay from '$lib/modules/iltavahti/IntensityOverlay.svelte';

	let sleepHoursLeft = 0;
	let intensityLevel: IntensityLevel = 'calm';
	let intensityColor = '#7eb2ff';
	let deadlineStr = '';
	let showInteraction = false;
	let currentInteraction: Interaction | null = null;
	let showOverlay = false;
	let ticker: ReturnType<typeof setInterval>;

	const unsubscribers: (() => void)[] = [];

	function tick() {
		const $s = $settings;
		sleepHoursLeft = sleepIfNow($s.wakeUpTime);
		const deadline = calculateDeadline($s.wakeUpTime, $s.sleepHours);
		const hoursToDeadline = hoursUntilDeadline(deadline);
		const newLevel = getIntensityLevel(hoursToDeadline);
		deadlineStr = formatTime(deadline);

		if (newLevel !== intensityLevel) {
			intensityLevel = newLevel;
			intensityColor = getIntensityColor(newLevel);
			handleIntensityChange(newLevel, hoursToDeadline, $s.intensityPreference);
		}
	}

	onMount(async () => {
		await requestPermission();

		tick();
		ticker = setInterval(tick, 1000);

		unsubscribers.push(
			on('interaction-required', async () => {
				currentInteraction = await pickInteraction();
				showInteraction = true;
				showOverlay = false;
			})
		);

		unsubscribers.push(
			on('intensity-changed', ({ level }) => {
				if (level === 'urgent' || level === 'overdue') {
					showOverlay = true;
				} else {
					showOverlay = false;
				}
			})
		);
	});

	onDestroy(() => {
		clearInterval(ticker);
		stopIntensityHandler();
		unsubscribers.forEach((u) => u());
	});

	function onInteractionDone() {
		showInteraction = false;
		currentInteraction = null;
	}
</script>

<div class="iltavahti" style="--current-color: {intensityColor}">
	<div class="content">
		<div class="main-display">
			<span class="big-number">{formatHoursMinutes(sleepHoursLeft)}</span>
			<span class="big-label">tuntia unta jäljellä</span>
		</div>

		<div class="info">
			<div class="info-row">
				<span class="info-label">Nukkumaanmeno</span>
				<span class="info-value">{deadlineStr}</span>
			</div>
			<div class="info-row">
				<span class="info-label">Herätys</span>
				<span class="info-value">{$settings.wakeUpTime}</span>
			</div>
			<div class="info-row">
				<span class="info-label">Taso</span>
				<span class="info-value intensity-badge" style="color: {intensityColor}">
					{intensityLevel}
				</span>
			</div>
		</div>
	</div>

	<IntensityOverlay {level}={intensityLevel} visible={showOverlay} />

	{#if showInteraction && currentInteraction}
		<InteractionModal interaction={currentInteraction} on:done={onInteractionDone} />
	{/if}
</div>

<style>
	.iltavahti {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--bg);
		transition: background-color 2s ease;
		position: relative;
	}

	.content {
		text-align: center;
		padding: 2rem;
		width: 100%;
		max-width: 400px;
	}

	.main-display {
		margin-bottom: 3rem;
	}

	.big-number {
		font-size: 6rem;
		font-weight: 200;
		font-variant-numeric: tabular-nums;
		display: block;
		color: var(--current-color);
		line-height: 1;
		transition: color 2s ease;
	}

	.big-label {
		font-size: 1rem;
		color: var(--text-dim);
		margin-top: 0.5rem;
		display: block;
	}

	.info {
		background: var(--bg-card);
		border: 1px solid var(--border-card);
		border-radius: var(--radius-lg);
		padding: 1.25rem;
	}

	.info-row {
		display: flex;
		justify-content: space-between;
		padding: 0.5rem 0;
	}
	.info-row + .info-row {
		border-top: 1px solid var(--border-card);
	}

	.info-label {
		color: var(--text-dim);
		font-size: 0.9rem;
	}
	.info-value {
		font-weight: 500;
		font-size: 0.9rem;
	}

	.intensity-badge {
		text-transform: capitalize;
		font-weight: 600;
	}
</style>
