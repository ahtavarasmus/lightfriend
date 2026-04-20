<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { settings, defaultSettings } from '$lib/core/state';
	import { loadSettings } from '$lib/core/storage';
	import '../app.css';

	let ready = false;

	onMount(async () => {
		const saved = await loadSettings();
		if (saved) {
			settings.set(saved);
		} else {
			settings.set(defaultSettings);
		}

		const s = saved || defaultSettings;
		if (!s.onboardingDone) {
			await goto('/tervetuloa');
		} else {
			await goto('/iltavahti');
		}
		ready = true;
	});
</script>

{#if ready}
	<slot />
{:else}
	<div class="loading">
		<p>Ladataan...</p>
	</div>
{/if}

<style>
	.loading {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100vh;
		color: var(--text-dim);
		font-size: 1.2rem;
	}
</style>
