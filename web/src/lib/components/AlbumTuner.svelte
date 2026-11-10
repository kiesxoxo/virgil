<script lang="ts" module>
	/** The album the dial was left on, for coming back to it. */
	let tuned: string | null = null;
	/** Lately picked by SEEK: they don't come up again for a while. */
	const picked: string[] = [];
</script>

<script lang="ts">
	import { Phone } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { albumHeader, albumItems } from '../actions.ts';
	import { line } from '../party.svelte.ts';
	import { queue } from '../request.svelte.ts';
	import { markDecoded } from '../artwork.ts';
	import { fitTitle } from '../fit.ts';
	import { TILE, artUrl, fold, formatShort, qualityRank } from '../format.ts';
	import { contextMenu, menu } from '../menu.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import Cover from './Cover.svelte';
	import Glyph from './Glyph.svelte';

	// The library as a radio band: one tick per album under a fixed needle. Drag or flick the dial and
	// the cover and title cut as the needle crosses each tick. SEEK jumps straight to a random album.
	// The dial is a native scroller, so a flick keeps iOS's own momentum; nothing else moves.
	let { albums, sort, reveal = null }: { albums: Album[]; sort: string; reveal?: string | null } = $props();

	/** How far apart the ticks are, in px. */
	const TICK = 22;
	const READOUT = 92;
	const TITLE = 112;
	/** Of the title's block, what isn't the title: its padding and the artist below. */
	const TITLE_CHROME = 48;
	const DIAL = 78;
	const KEYS = 60;
	/** Segments in the meter. */
	const LEVELS = 8;

	// Where the last view was, or the dial was left; freshly opened, anywhere on the band.
	let id = $state(untrack(() => reveal ?? tuned ?? albums[Math.floor(Math.random() * albums.length)]?.id ?? null));
	/** Where SEEK landed, until the dial moves. */
	let locked = $state(false);
	let width = $state(0);
	let height = $state(0);
	let scroller = $state<HTMLElement>();

	let indices = $derived(new Map(albums.map((album, i) => [album.id, i])));
	let index = $derived((id && indices.get(id)) || 0);
	let album = $derived(albums[index]);
	let size = $derived(Math.max(0, Math.floor(Math.min(width - READOUT, height - TITLE - DIAL - KEYS))));

	/** The meter's reading: the album's audio resolution, CD at about half. */
	let level = $derived.by(() => {
		const format = album?.format;
		if (!format) return 2;
		if (format.format === 'dsd') return LEVELS;
		return Math.max(1, Math.min(LEVELS, Math.round(Math.log2(qualityRank(format)) - 15.4)));
	});

	const initial = (text: string) => {
		const ch = fold(text).trim()[0]?.toUpperCase() ?? '';
		return /[A-Z]/.test(ch) ? ch : '#';
	};
	const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
	const month = (seconds: number) => {
		const date = new Date(seconds * 1000);
		return `${MONTHS[date.getMonth()]} ${String(date.getFullYear()).slice(2)}`;
	};
	/** The scale's words follow the sort: what to call an album, and how many ticks that word needs. */
	const scale: Record<string, { word: (album: Album) => string; ticks: number }> = {
		artist: { word: (a) => initial(a.artist), ticks: 1 },
		title: { word: (a) => initial(a.title), ticks: 1 },
		genre: { word: (a) => initial(a.genre ?? ''), ticks: 1 },
		year: { word: (a) => String(a.year ?? '—'), ticks: 3 },
		recent: { word: (a) => month(a.added), ticks: 3 }
	};

	let marks = $derived.by(() => {
		const words = scale[sort];
		let last = '';
		let room = 0;
		return albums.map((album, i) => {
			room--;
			if (!words) return { major: i % 10 === 0, word: '' };
			const word = words.word(album);
			if (word === last) return { major: false, word: '' };
			last = word;
			if (room > 0) return { major: true, word: '' };
			room = words.ticks;
			return { major: true, word };
		});
	});

	/** Puts the dial on album `i` at once: no snapping there. */
	function place(i: number) {
		const dial = scroller;
		if (!dial) return;
		dial.style.scrollSnapType = 'none';
		dial.scrollLeft = i * TICK;
		requestAnimationFrame(() => (dial.style.scrollSnapType = ''));
	}

	function tune(next: string) {
		id = next;
		tuned = next;
	}

	/** The needle crossed a tick. */
	function scrolled() {
		if (!scroller || !albums.length) return;
		const i = Math.max(0, Math.min(albums.length - 1, Math.round(scroller.scrollLeft / TICK)));
		if (i === index) return;
		tune(albums[i].id);
		locked = false;
		preload(i + 1, i - 1, i + 2, i - 2);
	}

	function step(by: number) {
		const i = Math.max(0, Math.min(albums.length - 1, index + by));
		if (i === index) return;
		tune(albums[i].id);
		locked = false;
		place(i);
		preload(i + by);
	}

	// The next album SEEK lands on is chosen ahead, and its cover decoded, so it's there at once.
	const fetched = new Set<string>();
	function fetchCover(album: Album | undefined) {
		const url = artUrl(album?.art, TILE);
		if (!url || fetched.has(url)) return;
		fetched.add(url);
		const image = new Image();
		image.src = url;
		image.decode().then(() => markDecoded(url), () => {});
	}
	function preload(...at: number[]) {
		for (const i of at) fetchCover(albums[i]);
	}

	function pick(): Album | undefined {
		const count = albums.length;
		if (count < 2) return undefined;
		let to = index;
		for (let tries = 0; tries < 20 && (to === index || picked.includes(albums[to].id)); tries++) to = Math.floor(Math.random() * count);
		if (to === index) to = (index + 1 + Math.floor(Math.random() * (count - 1))) % count;
		picked.push(albums[to].id);
		if (picked.length > Math.min(12, count / 2)) picked.shift();
		return albums[to];
	}

	let upcoming: Album | undefined;

	function seek() {
		const stillThere = upcoming && upcoming.id !== id && indices.has(upcoming.id);
		const to = stillThere ? upcoming : pick();
		if (!to) return;
		tune(to.id);
		locked = true;
		place(indices.get(to.id) ?? 0);
		upcoming = pick();
		fetchCover(upcoming);
	}

	// The list changed (a sort, a search) or the dial was just made: back on the album, or the nearest.
	$effect(() => {
		void albums;
		if (!scroller) return;
		const i = untrack(() => index);
		place(i);
		untrack(() => {
			if (albums[i]) tune(albums[i].id);
			preload(i + 1, i - 1);
			upcoming = pick();
			fetchCover(upcoming);
		});
	});

	// Desktop: arrow keys and a mouse wheel step the dial; a trackpad's sideways swipe just scrolls it.
	function keydown(e: KeyboardEvent) {
		if (menu.open || e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
		if (e.target instanceof HTMLElement && e.target.closest('input, textarea, select, [contenteditable]')) return;
		const by = { ArrowLeft: -1, ArrowRight: 1, Home: -albums.length, End: albums.length }[e.key];
		if (!by) return;
		e.preventDefault();
		step(by);
	}

	function wheel(node: HTMLElement) {
		let spun = 0;
		const onwheel = (e: WheelEvent) => {
			if (Math.abs(e.deltaX) >= Math.abs(e.deltaY)) return;
			e.preventDefault();
			spun += e.deltaY;
			const steps = Math.trunc(spun / 40);
			if (!steps) return;
			spun -= steps * 40;
			step(steps);
		};
		node.addEventListener('wheel', onwheel, { passive: false });
		return () => node.removeEventListener('wheel', onwheel);
	}

	const pad = (n: number) => String(n).padStart(3, '0');
</script>

<svelte:window onkeydown={keydown} />

{#if album}
	<div class="tuner flex flex-col" data-album={album.id} bind:clientWidth={width} bind:clientHeight={height} {@attach wheel}>
		<div class="flex shrink-0 border-b border-line" style:height="{size}px">
			<a
				href="/album/{album.id}"
				class="touch-menu block shrink-0"
				style:width="{size}px"
				aria-label="{album.title} by {album.artist}"
				{@attach contextMenu(
					() => albumItems(album),
					() => albumHeader(album)
				)}
			>
				<Cover art={album.art} size={TILE} eager />
			</a>
			<div class="flex min-w-0 flex-1 flex-col gap-2 border-l border-line px-2.5 py-3">
				<!-- The bars give way on a short screen, so the readout under them still fits beside the cover. -->
				<div class="flex min-h-0 max-w-40 flex-1 flex-col-reverse justify-end gap-1" role="meter" aria-label="Resolution" aria-valuemin={0} aria-valuemax={LEVELS} aria-valuenow={level}>
					{#each { length: LEVELS } as _, i (i)}
						<div class="max-h-[11px] min-h-0 flex-1 {i < level ? (locked ? 'bg-good' : 'bg-fg') : 'bg-surface-2'}"></div>
					{/each}
				</div>
				<div class="label shrink-0 text-[10px] leading-[17px] text-muted">
					<div class={locked ? 'text-good' : ''}>{locked ? 'Lock' : 'Tuned'}</div>
					<div class="text-fg">{pad(index + 1)}/{pad(albums.length)}</div>
					<div>{album.year ?? '—'}</div>
					<div>{album.tracks} tr</div>
					<div class="truncate">{album.format ? formatShort(album.format) : 'Mixed'}</div>
				</div>
			</div>
		</div>

		<a href="/album/{album.id}" class="flex min-h-0 flex-1 flex-col overflow-hidden px-3 py-2.5">
			<h2 class="wide -mb-[0.16em] line-clamp-3 min-h-0 pb-[0.16em] leading-[1.03]" style:font-size="{fitTitle(album.title, width - 24, { max: 56, lines: 3, tall: height - size - DIAL - KEYS - TITLE_CHROME })}px">{album.title}</h2>
			<p class="mt-1.5 shrink-0 truncate text-sm text-muted">{album.artist}</p>
		</a>

		<div class="relative shrink-0 border-t border-line" style:height="{DIAL}px">
			<!-- The padding lets the first and the last tick reach the needle. -->
			<div
				bind:this={scroller}
				class="flex h-full snap-x snap-mandatory overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
				style:padding-inline="calc(50% - {TICK / 2}px)"
				data-swipe
				tabindex="-1"
				onscroll={scrolled}
			>
				{#each marks as mark, i (i)}
					<div class="relative h-full shrink-0 snap-center" style:width="{TICK}px">
						<span class="absolute bottom-0 left-1/2 -ml-px w-0.5 {mark.major ? 'h-[34px] bg-fg' : 'h-5 bg-muted'}"></span>
						{#if mark.word}
							<span class="wide absolute top-2.5 left-1/2 -ml-px text-[13px] leading-4 whitespace-nowrap uppercase">{mark.word}</span>
						{/if}
					</div>
				{/each}
			</div>
			<div class="pointer-events-none absolute inset-y-0 left-1/2 -ml-[1.5px] w-[3px] bg-accent"></div>
		</div>

		<div class="grid shrink-0 grid-cols-2 border-t border-line" style:height="{KEYS}px">
			<button type="button" class="press-lit wide bg-fg text-[19px] text-bg disabled:bg-surface-2 disabled:text-faint" disabled={albums.length < 2} onclick={seek}>SEEK</button>
			<button type="button" class="press wide flex items-center justify-center gap-2.5 bg-accent text-[19px] text-accent-fg" onclick={() => queue.playAlbum(album.id)}>
				{#if line.open}<Phone class="size-4" strokeWidth={2.5} />REQUEST{:else}<Glyph name="play" class="size-4" />PLAY{/if}
			</button>
		</div>
	</div>
{/if}

<style>
	/*
	 * Fits between the controls and the player, so the page doesn't scroll under the dial.
	 * --masthead is the masthead's height: its word at 9:1 in the column, plus its padding.
	 */
	.tuner {
		--masthead: calc((100vw - var(--side-l) - var(--side-r) - 24px) / 9 + 22px);
		height: max(26rem, calc(100dvh - var(--top) - var(--masthead) - var(--libbar) - var(--nav-h) - 4rem - env(safe-area-inset-bottom)));
	}
	:global(:root[data-searching]) .tuner {
		--masthead: 0px;
	}
	@media (min-width: 1024px) {
		.tuner {
			--masthead: calc(min(56rem, 100vw - 15rem - 24px) / 9 + 22px);
			height: max(26rem, calc(100dvh - var(--masthead) - 85px - 72px));
		}
	}
</style>
