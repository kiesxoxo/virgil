<script lang="ts">
	import { untrack } from 'svelte';
	import { api } from '../api.ts';
	import { flight } from '../flight.svelte.ts';
	import { tilt } from '../tilt.ts';
	import Cover from './Cover.svelte';

	/**
	 * Now Playing's cover. It tilts towards the pointer (or with the phone), swiping it sideways
	 * skips to the next or previous song, and pulling it down closes Now Playing.
	 */
	let {
		id,
		art,
		src,
		playing,
		canPrevious,
		canNext,
		onclose
	}: {
		/** The queue item: a new one after a swipe slides in from the other side. */
		id: number;
		art: string | null | undefined;
		/** Radio artwork, instead of `art`. */
		src: string | null;
		playing: boolean;
		canPrevious: boolean;
		canNext: boolean;
		onclose: () => void;
	} = $props();

	let width = $state(0);
	/** How far the cover is pulled sideways, in px. */
	let drag = $state(0);
	/** How far it is pulled down, in px. */
	let pull = $state(0);
	/** The finger moves the cover: no easing, it follows. */
	let dragging = $state(false);
	/** Swiped away (1 to the left for next, -1 for previous), waiting for the new song. */
	let leaving = $state(0);

	let shown = untrack(() => id);
	$effect(() => {
		if (id === shown) return;
		shown = id;
		if (!leaving) return;
		// Jump to the other side, then ease in.
		dragging = true;
		drag = leaving * width * 0.6;
		leaving = 0;
		requestAnimationFrame(() =>
			requestAnimationFrame(() => {
				dragging = false;
				drag = 0;
			})
		);
	});

	function swipe(node: HTMLElement) {
		let start: { x: number; y: number; t: number; pointer: number } | null = null;
		/** Which way the finger moves the cover, once it is clear. */
		let way: 'sideways' | 'down' | null = null;
		/** At the top of the page, a pull down is the cover's, not the page's (which would only bounce). */
		const atTop = () => scrollY <= 0;

		const down = (e: PointerEvent) => {
			if (e.pointerType !== 'touch' || leaving) return;
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
				dragging = true;
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
			if (way === 'down') {
				dragging = false;
				// Far enough, or flicked: it flies from where it is to the mini player.
				if (dy > 120 || speed(dy) > 0.6) onclose();
				else pull = 0;
				return;
			}
			const flick = Math.abs(speed(dx)) > 0.6;
			if (way !== 'sideways') return;
			dragging = false;
			const next = dx < 0;
			if ((Math.abs(dx) > width * 0.3 || flick) && (next ? canNext : canPrevious)) {
				leaving = next ? 1 : -1;
				drag = -leaving * width * 1.2;
				void (next ? api.next() : api.previous());
				// No new song after all: bring the cover back.
				setTimeout(() => {
					if (leaving) [leaving, drag] = [0, 0];
				}, 2000);
			} else {
				drag = 0;
			}
		};
		const cancel = (e: PointerEvent) => {
			if (start?.pointer !== e.pointerId) return;
			start = null;
			if (way) [dragging, drag, pull] = [false, 0, 0];
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
	<div class="mover" class:dragging style:transform="translate({drag}px, {pull}px)">
		<div class="sleeve" class:grounded={flight.hides('now')} style:--scale={playing ? 1 : 0.94} {@attach tilt()}>
			<Cover {art} {src} size={1000} eager slot="now" class="w-full rounded-3xl shadow-cover ring-1 ring-line" />
			<div class="sheen rounded-3xl"></div>
		</div>
	</div>
</div>

<style>
	.mover {
		position: relative;
		transition: transform 300ms cubic-bezier(0.2, 0, 0, 1);
	}
	.dragging {
		transition: none;
	}
	.sleeve {
		position: relative;
		transform: perspective(1200px) rotateX(var(--tilt-x, 0deg)) rotateY(var(--tilt-y, 0deg)) scale(var(--scale));
		transition: transform 500ms cubic-bezier(0.2, 0, 0, 1);
	}
	/* While its cover flies here (lib/flight.svelte.ts), flat, as the flying cover lands: then it
	   leans with the phone again from there, rather than turning all at once as it lands. */
	.sleeve.grounded {
		transform: perspective(1200px) rotateX(0deg) rotateY(0deg) scale(var(--scale));
	}
	/* data-tilting is set by the tilt attachment, out of the compiler's sight. */
	.sleeve:global([data-tilting]) {
		transition-duration: 150ms;
	}
	.sheen {
		position: absolute;
		inset: 0;
		pointer-events: none;
		background: radial-gradient(circle at var(--sheen-x, 50%) var(--sheen-y, 50%), rgb(255 255 255 / 0.3), transparent 60%);
		mix-blend-mode: soft-light;
		opacity: 0;
		transition: opacity 300ms;
	}
	.sleeve:global([data-tilting]) .sheen {
		opacity: 1;
	}
	@media (prefers-reduced-motion: reduce) {
		.mover,
		.sleeve {
			transition: none;
		}
	}
</style>
