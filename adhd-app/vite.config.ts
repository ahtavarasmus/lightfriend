import { sveltekit } from '@sveltejs/kit/vite';
import { SvelteKitPWA } from '@vite-pwa/sveltekit';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit(),
		SvelteKitPWA({
			manifest: {
				name: 'Iltavahti',
				short_name: 'Iltavahti',
				description: 'ADHD-unihallinta',
				theme_color: '#111111',
				background_color: '#111111',
				display: 'standalone',
				orientation: 'portrait',
				icons: [
					{ src: 'icon-192.png', sizes: '192x192', type: 'image/png' },
					{ src: 'icon-512.png', sizes: '512x512', type: 'image/png' }
				]
			}
		})
	]
});
