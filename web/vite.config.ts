import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// `npm run dev` proxies the API to a running hifid: HIFI_URL=http://hifi.local npm run dev
const target = process.env.HIFI_URL ?? 'http://127.0.0.1:8080';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			// A single-page app: hifid serves index.html for every client-side route.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		proxy: {
			'/api': { target, ws: true },
			'/art': target
		}
	}
});
