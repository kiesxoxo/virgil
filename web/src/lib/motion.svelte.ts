// The phone's movement, for covers that tilt with it (lib/motion.ts turns it into a tilt). Listened
// to only while something on screen uses it, the page is visible and motion is wanted.

import { filter, step } from './motion.ts';
import { isBoolean, store, stored } from './stored.ts';

type Listener = (x: number, y: number) => void;
type Sensor = { requestPermission?: () => Promise<'granted' | 'denied'> };

const browser = typeof window !== 'undefined';
// Safari has no DeviceOrientationEvent outside a secure context (plain http on the LAN).
const sensor = browser ? (window.DeviceOrientationEvent as unknown as Sensor | undefined) : undefined;
const still = browser ? matchMedia('(prefers-reduced-motion: reduce)') : null;

class Motion {
	/** A phone, with a sensor this page may use. */
	readonly supported = browser && matchMedia('(pointer: coarse)').matches && !!sensor;
	/** iOS asks before letting a page have it. */
	readonly needsPermission = typeof sensor?.requestPermission === 'function';
	/** Wanted: by default only where there is nothing to ask. */
	enabled = $state(false);

	#listeners = new Set<Listener>();
	#filter = filter();
	#last = 0;
	#listening = false;

	constructor() {
		if (!this.supported) return;
		this.enabled = stored('motion', isBoolean, !this.needsPermission);
		document.addEventListener('visibilitychange', () => this.#update());
		still?.addEventListener('change', () => this.#update());
		// iOS may have forgotten it was allowed. Asking takes a tap; when it's still allowed, no prompt.
		if (this.enabled && this.needsPermission) addEventListener('pointerup', () => void this.#ask(), { once: true });
	}

	/** Turns motion on, asking first where the browser wants that (it must be called from a tap). */
	async enable(): Promise<boolean> {
		const granted = !this.needsPermission || (await this.#ask());
		this.#set(granted);
		return granted;
	}

	disable() {
		this.#set(false);
	}

	/** Calls `fn` with the tilt, -1…1 along the screen's x and y, as it changes; 0, 0 is level. */
	subscribe(fn: Listener): () => void {
		this.#listeners.add(fn);
		this.#update();
		return () => {
			this.#listeners.delete(fn);
			this.#update();
		};
	}

	#set(on: boolean) {
		this.enabled = on;
		store({ motion: on });
		this.#update();
	}

	async #ask(): Promise<boolean> {
		try {
			return (await sensor!.requestPermission!()) === 'granted';
		} catch {
			return false;
		}
	}

	#update() {
		const on = this.supported && this.enabled && !still?.matches && this.#listeners.size > 0 && document.visibilityState === 'visible';
		if (on === this.#listening) return;
		this.#listening = on;
		if (on) {
			// Starting afresh: level is however the phone is held now.
			this.#filter = filter();
			this.#last = 0;
			addEventListener('deviceorientation', this.#onorientation);
		} else {
			removeEventListener('deviceorientation', this.#onorientation);
			if (this.#filter.sent.some(Boolean)) for (const fn of this.#listeners) fn(0, 0);
		}
	}

	#onorientation = (e: DeviceOrientationEvent) => {
		if (e.beta === null || e.gamma === null) return;
		const dt = this.#last ? Math.min(100, e.timeStamp - this.#last) : 0;
		this.#last = e.timeStamp;
		const tilt = step(this.#filter, e.beta, e.gamma, screen.orientation?.angle ?? 0, dt);
		if (tilt) for (const fn of this.#listeners) fn(...tilt);
	};
}

export const motion = new Motion();
