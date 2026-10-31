<script lang="ts" module>
	/** The album in the middle, so coming back from its page (or to this view) lands on it again. */
	let centred: string | null = null;
</script>

<script lang="ts">
	import { Play } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { albumHeader, albumItems } from '../actions.ts';
	import { queue } from '../api.ts';
	import { contextMenu } from '../menu.svelte.ts';
	import { morph } from '../morph.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import Cover from './Cover.svelte';

	let { albums }: { albums: Album[] } = $props();

	/** Covers kept in the DOM on each side of the middle one. */
	const REACH = 10;
	/** How far the side covers turn towards the middle. */
	const ANGLE = 55;
	const TOP = 24;

	const still = matchMedia('(prefers-reduced-motion: reduce)');

	let scroller: HTMLDivElement;
	let width = $state(0);
	let innerHeight = $state(800);
	let scroll = $state(0);

	let cover = $derived(Math.round(Math.max(120, Math.min(width * 0.68, innerHeight * 0.42, 340))));
	/** Distance between neighbouring covers; the middle one gets `room` more on each side. */
	let slot = $derived(Math.round(cover * 0.42));
	let room = $derived(cover * 0.42);
	let reflection = $derived(Math.round(cover * 0.35));
	let position = $derived(scroll / slot);
	let current = $derived(Math.min(albums.length - 1, Math.max(0, Math.round(position))));
	let first = $derived(Math.max(0, Math.floor(position) - REACH));
	let last = $derived(Math.min(albums.length, Math.ceil(position) + REACH + 1));
	let album = $derived(albums[current]);

	// The same album stays in the middle when the list changes (search, sort) or the size does.
	$effect(() => {
		if (!width) return;
		const index = Math.max(0, albums.findIndex((a) => a.id === untrack(() => centred)));
		// Snapping off meanwhile: Chrome would re-snap to the cover it snapped to before the jump.
		scroller.style.scrollSnapType = 'none';
		scroller.scrollLeft = scroll = index * slot;
		const frame = requestAnimationFrame(() => (scroller.style.scrollSnapType = ''));
		return () => cancelAnimationFrame(frame);
	});

	function onscroll() {
		scroll = scroller.scrollLeft;
		centred = albums[Math.round(scroll / slot)]?.id ?? centred;
	}

	/** Where a smooth scroll is heading, so steps add up before it gets there. */
	let target = 0;

	function goTo(index: number) {
		target = Math.max(0, Math.min(albums.length - 1, index));
		scroller.scrollTo({ left: target * slot, behavior: still.matches ? 'instant' : 'smooth' });
	}

	function step(by: number) {
		goTo((Math.abs(target - position) < 1.5 ? target : current) + by);
	}

	function onkeydown(e: KeyboardEvent) {
		const to = { ArrowLeft: -1, ArrowRight: 1, Home: -albums.length, End: albums.length }[e.key];
		if (to === undefined) return;
		e.preventDefault();
		step(to);
		requestAnimationFrame(() => scroller.querySelector<HTMLElement>(`[data-index="${target}"] a`)?.focus({ preventScroll: true }));
	}

	// Arrow keys come from the focused cover link. A mouse wheel scrolls up and down: step
	// through the covers instead. (Sideways is the scroller's.)
	function controls(node: HTMLElement) {
		let sum = 0;
		const onwheel = (e: WheelEvent) => {
			if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
			e.preventDefault();
			sum += e.deltaY;
			if (Math.abs(sum) < 40) return;
			step(Math.sign(sum));
			sum = 0;
		};
		node.addEventListener('keydown', onkeydown);
		node.addEventListener('wheel', onwheel, { passive: false });
		return () => {
			node.removeEventListener('keydown', onkeydown);
			node.removeEventListener('wheel', onwheel);
		};
	}

	// bind:clientWidth reports a frame late; the first frame (and a view transition's snapshot of
	// it) needs the real width, or the covers start at their smallest.
	function measure(node: HTMLElement) {
		width = node.clientWidth;
		const observer = new ResizeObserver(() => (width = node.clientWidth));
		observer.observe(node);
		return () => observer.disconnect();
	}

	function place(index: number): string {
		const d = Math.max(-1, Math.min(1, index - position));
		return `translateX(${d * room}px) translateZ(${-Math.abs(d) * cover * 0.6}px) rotateY(${-d * ANGLE}deg)`;
	}
</script>

<svelte:window bind:innerHeight />

<div
	bind:this={scroller}
	{onscroll}
	{@attach measure}
	{@attach controls}
	class="-mx-4 snap-x snap-mandatory overflow-x-auto overflow-y-hidden [scrollbar-width:none] lg:-mx-8 [&::-webkit-scrollbar]:hidden"
	role="region"
	aria-roledescription="carousel"
	aria-label="Albums"
>
	<div
		class="relative"
		style:width="{width + (albums.length - 1) * slot}px"
		style:height="{TOP + cover + reflection}px"
	>
		{#each albums.slice(first, last) as item, i (item.id)}
			{@const index = first + i}
			{@const middle = index === current}
			{@const left = width / 2 - slot / 2 + index * slot}
			<!-- Perspective works on direct children only, so each slot has it, all seen from the middle. -->
			<div
				class="pointer-events-none absolute top-0 h-full snap-center"
				style:left="{left}px"
				style:width="{slot}px"
				style:z-index={1000 - Math.round(Math.abs(index - position) * 10)}
				style:perspective="{cover * 3}px"
				style:perspective-origin="{scroll + width / 2 - left}px {TOP + cover / 2}px"
				data-index={index}
			>
				<div
					class="pointer-events-auto absolute"
					style:top="{TOP}px"
					style:left="{(slot - cover) / 2}px"
					style:width="{cover}px"
					style:transform={place(index)}
					{@attach contextMenu(
						() => albumItems(item),
						() => albumHeader(item)
					)}
				>
					<a
						href="/album/{item.id}"
						class="block"
						tabindex={middle ? 0 : -1}
						aria-label="{item.title} by {item.artist}"
						onclick={(e) => {
							if (middle) return;
							e.preventDefault();
							goTo(index);
						}}
					>
						<Cover art={item.art} size={600} eager name={morph.name(item.id)} class="rounded-xl shadow-cover ring-1 ring-line" />
					</a>
					<div
						class="absolute inset-x-0 overflow-hidden rounded-xl"
						style:top="calc(100% + 4px)"
						style:height="{reflection}px"
						style:mask-image="linear-gradient(to bottom, rgb(0 0 0 / 0.3), transparent 85%)"
						aria-hidden="true"
					>
						<Cover art={item.art} size={600} class="-scale-y-100" />
					</div>
				</div>
			</div>
		{/each}
	</div>
</div>

{#if album}
	<div class="mt-1 flex flex-col items-center px-4 text-center" aria-live="polite">
		<a href="/album/{album.id}" class="max-w-full min-w-0">
			<div class="truncate text-base font-semibold">{album.title}</div>
			<div class="truncate text-sm text-muted">{album.artist}{album.year ? ` · ${album.year}` : ''}</div>
		</a>
		<button
			type="button"
			class="press mt-3 inline-flex h-10 items-center gap-1.5 rounded-full bg-accent pr-5 pl-4 text-sm font-semibold text-accent-fg shadow-float"
			onclick={() => queue.playAlbum(album.id)}
		>
			<Play class="size-4" fill="currentColor" /> Play
		</button>
	</div>
{/if}
