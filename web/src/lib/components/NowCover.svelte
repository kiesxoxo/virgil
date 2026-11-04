<script lang="ts">
	import { api } from '../api.ts';
	import Cover from './Cover.svelte';

	/**
	 * Now Playing's cover. Swiping it sideways skips to the next or previous song, and pulling it
	 * down closes Now Playing. It follows the finger, and is back in place the moment it lets go.
	 */
	let {
		art,
		src,
		canPrevious,
		canNext,
		onclose
	}: {
		art: string | null | undefined;
		/** Radio artwork, instead of `art`. */
		src: string | null;
		canPrevious: boolean;
		canNext: boolean;
		onclose: () => void;
	} = $props();

	let width = $state(0);
	/** How far the cover is pulled sideways, in px. */
	let drag = $state(0);
	/** How far it is pulled down, in px. */
	let pull = $state(0);

	function swipe(node: HTMLElement) {
		let start: { x: number; y: number; t: number; pointer: number } | null = null;
		/** Which way the finger moves the cover, once it is clear. */
		let way: 'sideways' | 'down' | null = null;
		/** At the top of the page, a pull down is the cover's, not the page's (which would only bounce). */
		const atTop = () => scrollY <= 0;

		const down = (e: PointerEvent) => {
			if (e.pointerType !== 'touch') return;
			start = { x: e.clientX, y: e.clientY, t: e.timeStamp, pointer: e.pointerId };
			way = null;
		};
		const move = (e: PointerEvent) => {
			if (start?.pointer !== e.pointerId) return;
			const dx = e.clientX - start.x;
			const dy = e.clientY - start.y;
			if (!way) {
				if (Math.max(Math.abs(dx), Math.abs(dy)) < 10) return;
				if (Math.abs(dx) >= Math.abs(dy)) way = 'sideways';
				else if (dy > 0 && atTop()) way = 'down';
				else return void (start = null); // scrolling the page
				node.setPointerCapture(e.pointerId);
			}
			if (way === 'down') pull = Math.max(0, dy);
			// Nothing to skip to that way: it gives, but reluctantly.
			else drag = (dx < 0 ? canNext : canPrevious) ? dx : dx * 0.25;
		};
		// The page mustn't scroll (or bounce) under a pull down: only a touchmove can stop that.
		const touchmove = (e: TouchEvent) => {
			if (!start || way === 'sideways') return;
			const touch = e.touches[0];
			const [dx, dy] = [touch.clientX - start.x, touch.clientY - start.y];
			if (way === 'down' || (dy > 0 && dy >= Math.abs(dx) && atTop())) e.preventDefault();
		};
		const up = (e: PointerEvent) => {
			if (start?.pointer !== e.pointerId) return;
			const dx = e.clientX - start.x;
			const dy = e.clientY - start.y;
			const time = Math.max(1, e.timeStamp - start.t);
			const speed = (d: number) => d / time;
			start = null;
			[drag, pull] = [0, 0];
			if (way === 'down') {
				if (dy > 120 || speed(dy) > 0.6) onclose();
			} else if (way === 'sideways' && (Math.abs(dx) > width * 0.3 || Math.abs(speed(dx)) > 0.6)) {
				if (dx < 0 && canNext) void api.next();
				else if (dx > 0 && canPrevious) void api.previous();
			}
		};
		const cancel = (e: PointerEvent) => {
			if (start?.pointer !== e.pointerId) return;
			start = null;
			[drag, pull] = [0, 0];
		};

		node.addEventListener('pointerdown', down);
		node.addEventListener('pointermove', move);
		node.addEventListener('pointerup', up);
		node.addEventListener('pointercancel', cancel);
		node.addEventListener('touchmove', touchmove, { passive: false });
		return () => {
			node.removeEventListener('touchmove', touchmove);
			node.removeEventListener('pointerdown', down);
			node.removeEventListener('pointermove', move);
			node.removeEventListener('pointerup', up);
			node.removeEventListener('pointercancel', cancel);
		};
	}
</script>

<div class="relative w-full touch-pan-y" data-swipe bind:clientWidth={width} {@attach swipe}>
	<div style:transform={drag || pull ? `translate(${drag}px, ${pull}px)` : undefined}>
		<Cover {art} {src} size={1000} eager class="w-full rounded-3xl shadow-cover ring-1 ring-line" />
	</div>
</div>
