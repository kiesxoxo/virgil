import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// `npm run dev` proxies the API to a running virgild: VIRGIL_URL=http://virgil.local npm run dev
const target = process.env.VIRGIL_URL ?? 'http://127.0.0.1:8080';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			// A single-page app: virgild serves index.html for every client-side route.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		proxy: {
			'/api': { target, ws: true },
			// Artwork only: a bare '/art' would also send /artists and /artist/… to virgild.
			'^/art/': target
		}
	}
});
