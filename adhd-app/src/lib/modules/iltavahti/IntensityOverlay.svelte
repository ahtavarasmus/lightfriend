<script lang="ts">
	import type { IntensityLevel } from '$lib/core/events';

	export let level: IntensityLevel;
	export let visible: boolean;

	const messages: Record<string, string> = {
		urgent: 'MENE NUKKUMAAN',
		overdue: 'OLET MYÖHÄSSÄ'
	};
</script>

{#if visible && (level === 'urgent' || level === 'overdue')}
	<div class="overlay" class:overdue={level === 'overdue'}>
		<div class="pulse-ring"></div>
		<h1>{messages[level]}</h1>
		<p>Suorita tehtävä jatkaaksesi</p>
	</div>
{/if}

<style>
	.overlay {
		position: fixed;
		inset: 0;
		background: rgba(255, 82, 82, 0.15);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		z-index: 900;
		animation: fadeIn 0.5s ease;
		pointer-events: none;
	}
	.overlay.overdue {
		background: rgba(224, 64, 251, 0.15);
	}

	h1 {
		font-size: 2rem;
		font-weight: 700;
		color: var(--intensity-urgent);
		text-align: center;
	}
	.overdue h1 {
		color: var(--intensity-overdue);
	}

	p {
		color: var(--text-dim);
		margin-top: 1rem;
		font-size: 1rem;
	}

	.pulse-ring {
		width: 200px;
		height: 200px;
		border-radius: 50%;
		border: 2px solid var(--intensity-urgent);
		animation: pulse 2s ease-in-out infinite;
		margin-bottom: 2rem;
	}
	.overdue .pulse-ring {
		border-color: var(--intensity-overdue);
	}

	@keyframes pulse {
		0%, 100% { transform: scale(0.8); opacity: 0.3; }
		50% { transform: scale(1.1); opacity: 0.8; }
	}

	@keyframes fadeIn {
		from { opacity: 0; }
		to { opacity: 1; }
	}
</style>
