// Which one-handed layout the phone uses, chosen in Settings (or with ?reach=deck, for trying one).
//
//   deck  Six tabs in one row, and each page's keys in a row right above the player.
//   rail  A column of keys down the thumb's edge of the screen, instead of the tab bar.
//   arc   A dial in the thumb's corner: press it and slide onto a key, instead of the tab bar.
//
// It shows as data-reach and data-hand on <html>, for app.css and the deck:, rail:, arc: and
// reach: variants. Larger screens have room and two hands, so they never get one.

import { store, stored } from './stored.ts';

export type ReachMode = 'off' | 'deck' | 'rail' | 'arc';
export type Hand = 'right' | 'left';

const isMode = (v: unknown): v is ReachMode => v === 'off' || v === 'deck' || v === 'rail' || v === 'arc';
const isHand = (v: unknown): v is Hand => v === 'right' || v === 'left';
const browser = typeof window !== 'undefined';
const phone = browser ? window.matchMedia('(max-width: 767.98px)') : null;

class Reach {
	mode = $state<ReachMode>(stored('reach', isMode, 'off'));
	hand = $state<Hand>(stored('hand', isHand, 'right'));
	/** Rail and arc: the sort words and view keys are showing, over the page. */
	panel = $state(false);
	#phone = $state(phone?.matches ?? false);

	/** The layout in force: this is a phone, and one is chosen. */
	active = $derived<ReachMode>(this.#phone ? this.mode : 'off');

	constructor() {
		if (!browser) return;
		const asked = new URLSearchParams(location.search).get('reach');
		if (isMode(asked)) this.set(asked);
		phone?.addEventListener('change', (e) => {
			this.#phone = e.matches;
			this.#apply();
		});
		this.#apply();
	}

	set(mode: ReachMode) {
		this.mode = mode;
		this.panel = false;
		store({ reach: mode });
		this.#apply();
	}

	setHand(hand: Hand) {
		this.hand = hand;
		store({ hand });
		this.#apply();
	}

	#apply() {
		const root = document.documentElement;
		const mode = phone?.matches ? this.mode : 'off';
		if (mode === 'off') delete root.dataset.reach;
		else root.dataset.reach = mode;
		root.dataset.hand = this.hand;
	}
}

export const reach = new Reach();
