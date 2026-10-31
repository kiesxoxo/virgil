<script lang="ts">
	import { untrack } from 'svelte';
	import { api } from '../api.ts';
	import { tilt } from '../tilt.ts';
	import Cover from './Cover.svelte';

	/**
	 * Now Playing's cover. It tilts towards the pointer (or with the phone), and swiping it sideways
	 * skips to the next or previous song.
	 */
	let {
		id,
		art,
		src,
		playing,
		canPrevious,
		canNext
	}: {
		/** The queue item: a new one after a swipe slides in from the other side. */
		id: number;
		art: string | null | undefined;
		/** Radio artwork, instead of `art`. */
		src: string | null;
		playing: boolean;
		canPrevious: boolean;
		canNext: boolean;
	} = $props();

	let width = $state(0);
	/** How far the cover is pulled sideways, in px. */
	let drag = $state(0);
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
		let sideways = false;

		const down = (e: PointerEvent) => {
			if (e.pointerType !== 'touch' || leaving) return;
			start = { x: e.clientX, y: e.clientY, t: e.timeStamp, pointer: e.pointerId };
			sideways = false;
		};
		const move = (e: PointerEvent) => {
			if (start?.pointer !== e.pointerId) return;
			const dx = e.clientX - start.x;
			if (!sideways) {
				if (Math.abs(e.clientY - start.y) > 10) return void (start = null); // scrolling the page
				if (Math.abs(dx) < 10) return;
				sideways = dragging = true;
				node.setPointerCapture(e.pointerId);
			}
			// Nothing to skip to that way: it gives, but reluctantly.
			drag = (dx < 0 ? canNext : canPrevious) ? dx : dx * 0.25;
		};
		const up = (e: PointerEvent) => {
			if (start?.pointer !== e.pointerId) return;
			const dx = e.clientX - start.x;
			const flick = Math.abs(dx) / Math.max(1, e.timeStamp - start.t) > 0.6;
			start = null;
			if (!sideways) return;
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
			if (sideways) [dragging, drag] = [false, 0];
		};

		node.addEventListener('pointerdown', down);
		node.addEventListener('pointermove', move);
		node.addEventListener('pointerup', up);
		node.addEventListener('pointercancel', cancel);
		return () => {
			node.removeEventListener('pointerdown', down);
			node.removeEventListener('pointermove', move);
			node.removeEventListener('pointerup', up);
			node.removeEventListener('pointercancel', cancel);
		};
	}
</script>

<div class="relative w-full touch-pan-y" bind:clientWidth={width} {@attach swipe}>
	<div class="mover" class:dragging style:transform="translateX({drag}px)">
		<div class="sleeve" style:--scale={playing ? 1 : 0.94} {@attach tilt()}>
			<Cover {art} {src} size={1000} eager name="now-cover" class="w-full rounded-3xl shadow-cover ring-1 ring-line" />
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
