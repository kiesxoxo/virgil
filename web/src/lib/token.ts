// This device's secret token: virgild maps it to the person using the device. It is made here
// (crypto.randomUUID needs HTTPS, which http://virgil.local isn't) and kept in localStorage.

const KEY = 'who';

function fresh(): string {
	const bytes = new Uint8Array(16);
	crypto.getRandomValues(bytes);
	return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

function load(): string {
	try {
		const saved = localStorage.getItem(KEY);
		if (saved && /^[0-9a-f]{32}$/.test(saved)) return saved;
		const token = fresh();
		localStorage.setItem(KEY, token);
		return token;
	} catch {
		// Storage blocked (private browsing): this visit gets a token of its own.
		return fresh();
	}
}

export const token = typeof window === 'undefined' ? '' : load();
