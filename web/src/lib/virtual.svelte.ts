// The rows of a window-scrolled list or grid to have in the DOM (lib/virtual.ts), kept up to date
// with the page's scroll. A list is laid out where it will be from the start: its width is known on
// its first render, it opens on its item, and it follows the scroll SvelteKit puts back on the way
// back to it, before anything looks at it (a cover flying home to it, lib/flight.svelte.ts).

import { afterNavigate } from '$app/navigation';
import { untrack } from 'svelte';
import { revealScroll, rowWindow, topRow } from './virtual.ts';

interface Spec {
	count: number;
	/** Items to a row. */
	columns?: number;
	rowHeight: number;
	/** Rows kept beyond those in sight, each way. */
	overscan: number;
	/** The item to open on in the middle of the screen, and how tall its picture is at the top of its row (read once, as the list comes in). */
	reveal?: { index: number; height: number } | null;
}

export class Rows {
	/** The list's width. */
	width = $state(0);
	/** Measured, and where it opens: the rows can go in (rather than the first ones, for a moment). */
	ready = $state(false);
	#viewport = $state(typeof window === 'undefined' ? 800 : innerHeight);
	/** The row at the top of the screen. */
	#top = $state(0);
	#spec: () => Spec;
	#node: HTMLElement | null = null;

	constructor(spec: () => Spec) {
		this.#spec = spec;
		// SvelteKit has put the scroll back by now, coming back to the page.
		afterNavigate(() => this.measure());
		// Content above the list can change height (search results): measure again with the data.
		$effect(() => {
			const { count, rowHeight } = this.#spec();
			void [count, rowHeight];
			untrack(() => this.measure());
		});
	}

	#window = $derived.by(() => {
		const { count, columns = 1, rowHeight, overscan } = this.#spec();
		return rowWindow(this.#top, this.#viewport, rowHeight, Math.ceil(count / columns), overscan);
	});

	/** The first row in the DOM. */
	get first(): number {
		return this.#window.first;
	}

	/** The row after the last in the DOM. */
	get last(): number {
		return this.#window.last;
	}

	/** Which row is at the top of the screen. */
	measure() {
		if (!this.#node) return;
		this.#top = topRow(-this.#node.getBoundingClientRect().top, this.#spec().rowHeight);
	}

	/** The list's element: measured at once (and its first render laid out at its width), opened on its item. */
	attach = (node: HTMLElement) =>
		untrack(() => {
			this.#node = node;
			this.width = node.clientWidth;
			const { reveal, columns = 1, rowHeight } = this.#spec();
			if (reveal && reveal.index >= 0) {
				const top = node.getBoundingClientRect().top + scrollY;
				const row = Math.floor(reveal.index / columns);
				scrollTo({ top: revealScroll(top, row, rowHeight, reveal.height, innerHeight), behavior: 'instant' });
			}
			this.measure();
			this.ready = true;

			const resized = new ResizeObserver(() => (this.width = node.clientWidth));
			resized.observe(node);
			const scrolled = () => this.measure();
			const viewport = () => {
				this.#viewport = innerHeight;
				this.measure();
			};
			addEventListener('scroll', scrolled, { passive: true });
			addEventListener('resize', viewport, { passive: true });
			return () => {
				resized.disconnect();
				removeEventListener('scroll', scrolled);
				removeEventListener('resize', viewport);
				if (this.#node === node) [this.#node, this.ready] = [null, false];
			};
		});
}
