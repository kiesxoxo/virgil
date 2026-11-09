export type ThemeMode = 'system' | 'light' | 'dark';

const media = typeof window === 'undefined' ? null : window.matchMedia('(prefers-color-scheme: dark)');

function stored(): ThemeMode {
	try {
		const mode = localStorage.getItem('theme');
		if (mode === 'light' || mode === 'dark') return mode;
	} catch {
		/* storage blocked */
	}
	return 'system';
}

class Theme {
	mode = $state<ThemeMode>(stored());

	constructor() {
		media?.addEventListener('change', () => this.apply());
		this.apply();
	}

	set(mode: ThemeMode) {
		this.mode = mode;
		try {
			localStorage.setItem('theme', mode);
		} catch {
			/* storage blocked */
		}
		this.apply();
	}

	apply() {
		if (typeof document === 'undefined') return;
		const dark = this.mode === 'dark' || (this.mode === 'system' && !!media?.matches);
		const root = document.documentElement;
		root.classList.toggle('dark', dark);
		root.style.colorScheme = dark ? 'dark' : 'light';
		document.querySelector('meta[name="theme-color"]')?.setAttribute('content', dark ? '#000000' : '#eceae4');
	}
}

export const theme = new Theme();
