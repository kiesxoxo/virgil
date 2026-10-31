<script lang="ts" module>
	/** The album in the middle, so coming back from its page (or to this view) lands on it again. */
	let centred: string | null = null;
</script>

<script lang="ts">
	import { Play } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { albumHeader, albumItems } from '../actions.ts';
	import { queue } from '../api.ts';
	import { TOP, hitTest, layout } from '../coverflow.ts';
	import { flowRenderer, readTheme, type Scene } from '../coverflow-gl.ts';
	import { contextMenu } from '../menu.svelte.ts';
	import { morph } from '../morph.svelte.ts';
	import { theme } from '../theme.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import AlbumGrid from './AlbumGrid.svelte';
	import Cover from './Cover.svelte';

	let { albums }: { albums: Album[] } = $props();

	/** Snap points kept in the DOM on each side of the middle cover. */
	const REACH = 10;

	const still = matchMedia('(prefers-reduced-motion: reduce)');
	// The covers are drawn with WebGL (lib/coverflow-gl.ts), straight from the scroll position:
	// scrolling changes nothing here but the cover in the middle, once per cover.
	const renderer = flowRenderer();

	let scroller = $state() as HTMLDivElement;
	let link = $state<HTMLAnchorElement>();
	let width = $state(0);
	let innerHeight = $state(800);
	/** The page's height that isn't for the covers: what is above them, and the bars below. */
	let chrome = $state(0);
	let current = $state(0);
	/** At rest on the middle cover, not between two. */
	let settled = $state(true);

	let l = $derived(layout(width, innerHeight - chrome));
	let snaps = $derived.by(() => {
		const first = Math.max(0, current - REACH);
		return Array.from({ length: Math.min(albums.length, current + REACH + 1) - first }, (_, i) => first + i);
	});
	let album = $derived(albums[current]);
	/** The cover a page transition moves: a real image over the canvas, so it can fly. */
	let moving = $derived(settled && !!album && morph.album === album.id);

	const position = () => scroller.scrollLeft / l.slot;

	// Untracked: drawing at once from an effect must not make the effect depend on the scroll.
	const scene = (): Scene =>
		untrack(() => {
			const list = albums;
			return {
				layout: l,
				position: scroller ? position() : current,
				count: list.length,
				art: (index) => list[index]?.art ?? null,
				hidden: moving ? current : -1,
				settled
			};
		});

	// The same album stays in the middle when the list changes (search, sort) or the size does.
	$effect(() => {
		if (!width) return;
		const index = Math.max(0, albums.findIndex((a) => a.id === untrack(() => centred)));
		// Snapping off meanwhile: Chrome would re-snap to the cover it snapped to before the jump.
		scroller.style.scrollSnapType = 'none';
		scroller.scrollLeft = index * l.slot;
		current = index;
		settled = true;
		// Now, not on the next frame: a page transition may be taking its picture of this one.
		renderer?.draw();
		const frame = requestAnimationFrame(() => (scroller.style.scrollSnapType = ''));
		return () => cancelAnimationFrame(frame);
	});

	$effect(() => {
		void [l, albums, moving, settled];
		renderer?.invalidate();
	});

	$effect(() => {
		void theme.dark;
		renderer?.setTheme(readTheme());
	});

	function onscroll() {
		const left = scroller.scrollLeft;
		const index = Math.min(albums.length - 1, Math.max(0, Math.round(left / l.slot)));
		current = index;
		settled = Math.abs(left - index * l.slot) < 1;
		centred = albums[index]?.id ?? centred;
		renderer?.invalidate();
	}

	/** Where a smooth scroll is heading, so steps add up before it gets there. */
	let target = 0;

	function goTo(index: number) {
		target = Math.max(0, Math.min(albums.length - 1, index));
		scroller.scrollTo({ left: target * l.slot, behavior: still.matches ? 'instant' : 'smooth' });
	}

	function step(by: number) {
		goTo((Math.abs(target - position()) < 1.5 ? target : current) + by);
	}

	function onkeydown(e: KeyboardEvent) {
		const to = { ArrowLeft: -1, ArrowRight: 1, Home: -albums.length, End: albums.length }[e.key];
		if (to === undefined) return;
		e.preventDefault();
		step(to);
		link?.focus({ preventScroll: true });
	}

	/** The cover under the pointer, or -1. */
	function hit(e: MouseEvent): number {
		const r = scroller.getBoundingClientRect();
		return hitTest(l, position(), albums.length, e.clientX - r.left, e.clientY - r.top);
	}

	// The middle cover is a link of its own; a side cover comes to the middle.
	function onclick(e: MouseEvent) {
		if (link?.contains(e.target as Node)) return;
		const index = hit(e);
		if (index === current) goto(`/album/${albums[index].id}`);
		else if (index >= 0) goTo(index);
	}

	/** The album a long press or right-click is on: the one under it, or the middle one. */
	let pressed = -1;
	const menuAlbum = () => albums[pressed] ?? album;

	// Arrow keys come from the middle cover's link. A mouse wheel scrolls up and down: step
	// through the covers instead. (Sideways is the scroller's.) Clicks and taps go to the cover
	// under them.
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
		// Captured, so it is known before the context menu asks.
		const onpress = (e: MouseEvent) => (pressed = hit(e));
		node.addEventListener('keydown', onkeydown);
		node.addEventListener('click', onclick);
		node.addEventListener('wheel', onwheel, { passive: false });
		node.addEventListener('pointerdown', onpress, true);
		node.addEventListener('contextmenu', onpress, true);
		return () => {
			node.removeEventListener('keydown', onkeydown);
			node.removeEventListener('click', onclick);
			node.removeEventListener('wheel', onwheel);
			node.removeEventListener('pointerdown', onpress, true);
			node.removeEventListener('contextmenu', onpress, true);
		};
	}

	// bind:clientWidth reports a frame late; the first frame (and a view transition's snapshot of
	// it) needs the real width, or the covers start at their smallest.
	function measure(node: HTMLElement) {
		const update = () => {
			width = node.clientWidth;
			// The main element's bottom padding keeps the page clear of the player bar (and tab bar).
			const bars = parseFloat(getComputedStyle(node.closest('main') ?? node).paddingBottom) || 0;
			chrome = node.getBoundingClientRect().top + scrollY + bars;
		};
		update();
		const observer = new ResizeObserver(update);
		observer.observe(node);
		return () => observer.disconnect();
	}

	// The renderer's canvas lies under the scroller, which takes the touches.
	function canvas(node: HTMLElement) {
		if (!renderer) return;
		node.prepend(renderer.canvas);
		renderer.show(scene);
		return () => {
			renderer.hide();
			renderer.canvas.remove();
		};
	}
</script>

<svelte:window bind:innerHeight />

{#if renderer}
	<div class="relative -mx-4 lg:-mx-8" {@attach canvas}>
		<div
			bind:this={scroller}
			{onscroll}
			{@attach measure}
			{@attach controls}
			{@attach contextMenu(
				() => albumItems(menuAlbum()),
				() => albumHeader(menuAlbum())
			)}
			class="relative snap-x snap-mandatory overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
			role="region"
			aria-roledescription="carousel"
			aria-label="Albums"
		>
			<div class="relative" style:width="{width + (albums.length - 1) * l.slot}px" style:height="{l.height}px">
				{#each snaps as index (index)}
					<div class="absolute top-0 h-full snap-center" style:left="{width / 2 - l.slot / 2 + index * l.slot}px" style:width="{l.slot}px"></div>
				{/each}
				{#if album}
					<!-- Where the middle cover is drawn when at rest. At rest it also holds the cover as an
					     image, shown when a page transition moves it (the canvas leaves that one out). -->
					<a
						bind:this={link}
						href="/album/{album.id}"
						class="absolute block rounded-xl"
						style:left="{width / 2 - l.cover / 2 + current * l.slot}px"
						style:top="{TOP}px"
						style:width="{l.cover}px"
						style:height="{l.cover}px"
						aria-label="{album.title} by {album.artist}"
					>
						{#if settled}
							<Cover
								art={album.art}
								size={1000}
								eager
								name={morph.name(album.id)}
								class="rounded-xl shadow-cover ring-1 ring-line {moving ? '' : 'opacity-0'}"
							/>
						{/if}
					</a>
				{/if}
			</div>
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
{:else}
	<!-- No WebGL after all (the page offers the flow only where there is): the grid instead. -->
	<AlbumGrid {albums} />
{/if}
