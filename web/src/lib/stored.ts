// View preferences kept in localStorage (sort order, grid or list). Storage
// can be blocked (private browsing) or hold an older format: then the
// fallback is used and nothing is saved, without breaking the page.

/** The value saved under `key`, if it passes `valid`; otherwise `fallback`. */
export function stored<T>(key: string, valid: (v: unknown) => v is T, fallback: T): T {
	try {
		const v = JSON.parse(localStorage.getItem(key) ?? 'null');
		if (valid(v)) return v;
	} catch {
		/* storage blocked or old format */
	}
	return fallback;
}

/** Saves each value under its key. */
export function store(values: Record<string, unknown>) {
	try {
		for (const [key, value] of Object.entries(values)) localStorage.setItem(key, JSON.stringify(value));
	} catch {
		/* storage blocked */
	}
}

export const isView = (v: unknown): v is 'grid' | 'list' => v === 'grid' || v === 'list';
export const isBoolean = (v: unknown): v is boolean => typeof v === 'boolean';
