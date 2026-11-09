// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
	namespace App {
		interface PageState {
			/** The route this history entry was opened from; null when that wasn't a page in the app (lib/back.ts). */
			opener?: string | null;
		}
	}
}

export {};
