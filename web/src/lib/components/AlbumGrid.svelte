<script lang="ts">
	import { untrack } from 'svelte';
	import { albumHeader, albumItems } from '../actions.ts';
	import { queue } from '../request.svelte.ts';
	import { TILE } from '../format.ts';
	import { library } from '../library.svelte.ts';
	import { contextMenu } from '../menu.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import { Rows } from '../virtual.svelte.ts';
	import Cover from './Cover.svelte';
	import Glyph from './Glyph.svelte';

	/** `reveal`: the album to open on (in the middle of the screen in the view before). */
	let { albums, reveal = null }: { albums: Album[]; reveal?: string | null } = $props();

	// Window-scrolled virtual grid: only rows near the viewport exist in the DOM. Covers butt up edge
	// to edge, divided by rules, like a record shop's catalogue.
	/** The title (two lines at most), the artist, and the rule under them. */
	const META = 72;
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
	let minTile = $derived(width < 640 ? 140 : 180);
	let cols: number = $derived(Math.max(2, Math.floor(width / minTile)));
	// Whole pixels, so that no cover lands between two; what's left over stays by the last column.
	let tile = $derived(Math.max(0, Math.floor(width / cols)));
	let rowHeight = $derived(tile + META);
	let rows = $derived(Math.ceil(albums.length / cols));
	let visible = $derived(grid.ready ? albums.slice(grid.first * cols, grid.last * cols) : []);
</script>

<div {@attach grid.attach} class="relative" style:height="{Math.max(0, rows * rowHeight)}px">
	{#each visible as album, i (album.id)}
		{@const index = grid.first * cols + i}
		{@const versions = library.versions(album.id).length}
		<div
			class="group absolute top-0 left-0 border-b border-line {index % cols < cols - 1 ? 'rule-r' : ''}"
			data-album={album.id}
			style:width="{tile}px"
			style:height="{rowHeight}px"
			style:transform="translate({(index % cols) * tile}px, {Math.floor(index / cols) * rowHeight}px)"
			{@attach contextMenu(
				() => albumItems(album),
				() => albumHeader(album)
			)}
		>
			<a href="/album/{album.id}" class="block">
				<div class="relative">
					<Cover art={album.art} size={TILE} />
					{#if versions > 1}
						<span class="label absolute top-0 left-0 bg-black px-1.5 py-1 text-[10px] text-white" title="{versions} versions">×{versions}</span>
					{/if}
				</div>
				<div class="px-2.5 pt-2">
					<div class="wide line-clamp-2 text-[12.5px] leading-[15px] [overflow-wrap:anywhere]">{album.title}</div>
					<div class="mt-0.5 truncate text-xs leading-4 text-muted">{album.artist}</div>
				</div>
			</a>
			<button
				type="button"
				class="press absolute right-0 grid size-11 place-items-center bg-accent text-accent-fg opacity-0 group-hover:opacity-100 focus-visible:opacity-100 max-lg:hidden"
				style:top="{tile - 44}px"
				aria-label="Play {album.title}"
				onclick={() => queue.playAlbum(album.id)}
			>
				<Glyph name="play" class="size-5" />
			</button>
		</div>
	{/each}
</div>
