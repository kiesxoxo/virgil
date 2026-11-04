<script lang="ts" module>
	/** Grid template for the detail columns; the albums page uses it for the header too. */
	export function listColumns(art: boolean): string {
		return `${art ? '3.5rem ' : ''}minmax(0,3fr) minmax(0,2fr) 3.25rem minmax(0,1.3fr) 3.25rem 4.5rem 5rem`;
	}

	/** Width at which the list switches from stacked rows to columns. */
	export const LIST_COLUMNS_MIN_WIDTH = 720;
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { albumHeader, albumItems } from '../actions.ts';
	import { clock, formatShort } from '../format.ts';
	import { hub } from '../hub.svelte.ts';
	import { library } from '../library.svelte.ts';
	import { contextMenu } from '../menu.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import { Rows } from '../virtual.svelte.ts';
	import Cover from './Cover.svelte';
	import Equalizer from './Equalizer.svelte';
	import Menu from './Menu.svelte';

	/** `reveal`: the album to open on (in the middle of the screen in the view before). */
	let { albums, art, wide, reveal = null }: { albums: Album[]; art: boolean; wide: boolean; reveal?: string | null } = $props();

	// Window-scrolled virtual list: only rows near the viewport exist in the DOM.
	const OVERSCAN = 4;

	const list = new Rows(() => ({
		count: albums.length,
		rowHeight,
		overscan: OVERSCAN,
		// Its row in the middle of the screen.
		reveal: reveal ? { index: untrack(() => albums.findIndex((a) => a.id === reveal)), height: rowHeight } : null
	}));

	let rowHeight = $derived(art ? 56 : wide ? 40 : 52);
	let visible = $derived(list.ready ? albums.slice(list.first, list.last) : []);
	let playingAlbum = $derived(hub.current?.albumId ?? null);
</script>

{#snippet versionsTag(album: Album)}
	{@const versions = library.versions(album.id).length}
	{#if versions > 1}
		<span class="label shrink-0 text-[10px] text-muted">×{versions}</span>
	{/if}
{/snippet}

{#snippet quality(album: Album)}
	<span class="label text-[11px] text-muted">{album.format ? formatShort(album.format) : 'Mixed'}</span>
{/snippet}

<div
	{@attach list.attach}
	class="relative"
	role="table"
	aria-label="Albums"
	aria-rowcount={albums.length}
	style:height="{albums.length * rowHeight}px"
>
	{#each visible as album, i (album.id)}
		{@const index = list.first + i}
		{@const playing = album.id === playingAlbum}
		<div
			role="row"
			data-album={album.id}
			aria-rowindex={index + 1}
			class="absolute inset-x-0 top-0 flex items-stretch border-b border-line hover:bg-surface"
			style:height="{rowHeight}px"
			style:transform="translateY({index * rowHeight}px)"
			{@attach contextMenu(
				() => albumItems(album),
				() => albumHeader(album)
			)}
		>
			<a
				href="/album/{album.id}"
				class="min-w-0 flex-1 {art ? '' : 'pl-3'} {wide ? 'grid items-center gap-x-4' : 'flex items-center gap-3'}"
				style:grid-template-columns={wide ? listColumns(art) : undefined}
			>
				{#if art}
					<Cover art={album.art} class="h-full shrink-0" />
				{/if}
				{#if wide}
					<span role="cell" class="flex min-w-0 items-center gap-2 {playing ? 'text-accent' : ''}">
						{#if playing}<Equalizer playing={hub.playing} />{/if}
						<span class="wide truncate text-[13px]">{album.title}</span>
						{@render versionsTag(album)}
					</span>
					<span role="cell" class="truncate text-[13px] text-muted">{album.artist}</span>
					<span role="cell" class="label text-[11px] text-muted">{album.year ?? ''}</span>
					<span role="cell" class="truncate text-[13px] text-muted">{album.genre ?? ''}</span>
					<span role="cell" class="label text-right text-[11px] text-muted">{album.tracks}</span>
					<span role="cell" class="label text-right text-[11px] text-muted">{clock(album.duration)}</span>
					<span role="cell" class="text-right">{@render quality(album)}</span>
				{:else}
					<span role="cell" class="min-w-0 flex-1">
						<span class="flex items-center gap-2 {playing ? 'text-accent' : ''}">
							{#if playing}<Equalizer playing={hub.playing} />{/if}
							<span class="wide truncate text-[13px] leading-[18px]">{album.title}</span>
							{@render versionsTag(album)}
						</span>
						<span class="block truncate text-xs leading-4 text-muted">{album.artist}</span>
					</span>
					<span role="cell" class="flex shrink-0 flex-col items-end leading-4">
						<span class="label text-[11px] text-muted">{album.year ?? ''}</span>
						{@render quality(album)}
					</span>
				{/if}
			</a>
			<div role="cell" class="flex shrink-0">
				<Menu items={() => albumItems(album)} header={albumHeader(album)} />
			</div>
		</div>
	{/each}
</div>
