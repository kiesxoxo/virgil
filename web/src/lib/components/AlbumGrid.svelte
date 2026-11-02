<script lang="ts">
	import { Layers, Play } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { albumHeader, albumItems } from '../actions.ts';
	import { queue } from '../api.ts';
	import { TILE } from '../format.ts';
	import { library } from '../library.svelte.ts';
	import { contextMenu } from '../menu.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import { Rows } from '../virtual.svelte.ts';
	import Cover from './Cover.svelte';

	/** `reveal`: the album to open on (in the middle of the screen in the view before). */
	let { albums, reveal = null }: { albums: Album[]; reveal?: string | null } = $props();

	// Window-scrolled virtual grid: only rows near the viewport exist in the DOM.
	const META = 46;
	const GAP_Y = 22;
	const OVERSCAN = 3;

	const grid: Rows = new Rows(() => ({
		count: albums.length,
		columns: cols,
		rowHeight,
		overscan: OVERSCAN,
		// Its row in the middle of the screen.
		reveal: reveal ? { index: untrack(() => albums.findIndex((a) => a.id === reveal)), height: tile } : null
	}));

	let width: number = $derived(grid.width);
	let gapX = $derived(width < 640 ? 14 : 22);
	let minTile = $derived(width < 640 ? 140 : 172);
	let cols: number = $derived(Math.max(2, Math.floor((width + gapX) / (minTile + gapX))));
	let tile = $derived(Math.max(0, (width - gapX * (cols - 1)) / cols));
	let rowHeight = $derived(tile + META + GAP_Y);
	let rows = $derived(Math.ceil(albums.length / cols));
	let visible = $derived(grid.ready ? albums.slice(grid.first * cols, grid.last * cols) : []);
</script>

<div {@attach grid.attach} class="relative" style:height="{Math.max(0, rows * rowHeight - GAP_Y)}px">
	{#each visible as album, i (album.id)}
		{@const index = grid.first * cols + i}
		{@const versions = library.versions(album.id).length}
		<div
			class="group absolute top-0 left-0"
			style:width="{tile}px"
			style:transform="translate3d({(index % cols) * (tile + gapX)}px, {Math.floor(index / cols) * rowHeight}px, 0)"
			{@attach contextMenu(
				() => albumItems(album),
				() => albumHeader(album)
			)}
		>
			<a href="/album/{album.id}" class="block press">
				<div class="relative">
					<Cover art={album.art} size={TILE} slot="album:{album.id}" class="rounded-xl shadow-cover ring-1 ring-line" />
					{#if versions > 1}
						<span
							class="absolute top-2 left-2 inline-flex items-center gap-1 rounded-full bg-black/60 px-1.5 py-0.5 text-[10px] font-semibold text-white"
							title="{versions} versions"
						>
							<Layers class="size-3" />{versions}
						</span>
					{/if}
				</div>
				<div class="mt-2.5 truncate text-[13px] leading-5 font-semibold">{album.title}</div>
				<div class="truncate text-[13px] leading-5 text-muted">{album.artist}</div>
			</a>
			<button
				type="button"
				class="absolute right-2.5 grid size-11 place-items-center rounded-full bg-accent text-accent-fg opacity-0 shadow-float transition duration-200 group-hover:opacity-100 hover:scale-105 focus-visible:opacity-100 max-lg:hidden"
				style:top="{tile - 54}px"
				aria-label="Play {album.title}"
				onclick={() => queue.playAlbum(album.id)}
			>
				<Play class="ml-0.5 size-5" fill="currentColor" />
			</button>
		</div>
	{/each}
</div>
