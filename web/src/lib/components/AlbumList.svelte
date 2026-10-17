<script lang="ts" module>
	/** Grid template for the detail columns; the albums page uses it for the header too. */
	export function listColumns(art: boolean): string {
		return `${art ? '2.5rem ' : ''}minmax(0,3fr) minmax(0,2fr) 3.25rem minmax(0,1.3fr) 3.25rem 4.5rem 5rem`;
	}

	/** Width at which the list switches from stacked rows to columns. */
	export const LIST_COLUMNS_MIN_WIDTH = 720;
</script>

<script lang="ts">
	import { albumHeader, albumItems } from '../actions.ts';
	import { clock, formatShort } from '../format.ts';
	import { hub } from '../hub.svelte.ts';
	import { library } from '../library.svelte.ts';
	import { contextMenu } from '../menu.svelte.ts';
	import type { Album } from '../types/Album.ts';
	import Cover from './Cover.svelte';
	import Equalizer from './Equalizer.svelte';
	import Menu from './Menu.svelte';

	let { albums, art, wide }: { albums: Album[]; art: boolean; wide: boolean } = $props();

	// Window-scrolled virtual list: only rows near the viewport exist in the DOM.
	const OVERSCAN = 10;

	let list: HTMLDivElement;
	let offset = $state(0);
	let viewport = $state(800);

	let rowHeight = $derived(art ? 56 : wide ? 40 : 52);
	let first = $derived(Math.max(0, Math.floor(offset / rowHeight) - OVERSCAN));
	let last = $derived(Math.min(albums.length, Math.ceil((offset + viewport) / rowHeight) + OVERSCAN));
	let visible = $derived(albums.slice(first, last));
	let playingAlbum = $derived(hub.current?.albumId ?? null);

	function measure() {
		if (list) offset = -list.getBoundingClientRect().top;
	}

	$effect(() => {
		void albums.length;
		void rowHeight;
		measure();
	});
</script>

<svelte:window onscroll={measure} onresize={measure} bind:innerHeight={viewport} />

{#snippet versionsTag(album: Album)}
	{@const versions = library.versions(album.id).length}
	{#if versions > 1}
		<span class="shrink-0 rounded-md bg-surface-2 px-1.5 py-px text-[10px] font-semibold text-muted">{versions} versions</span>
	{/if}
{/snippet}

{#snippet quality(album: Album)}
	{#if album.format}
		<span class="text-xs font-semibold tabular-nums text-muted">
			{formatShort(album.format)}
		</span>
	{:else}
		<span class="text-xs text-faint">Mixed</span>
	{/if}
{/snippet}

<div
	bind:this={list}
	class="relative"
	role="table"
	aria-label="Albums"
	aria-rowcount={albums.length}
	style:height="{albums.length * rowHeight}px"
>
	{#each visible as album, i (album.id)}
		{@const index = first + i}
		{@const playing = album.id === playingAlbum}
		<div
			role="row"
			aria-rowindex={index + 1}
			class="absolute inset-x-0 top-0 flex items-center rounded-lg hover:bg-fg/4 {playing ? 'bg-accent/6' : ''}"
			style:height="{rowHeight}px"
			style:transform="translateY({index * rowHeight}px)"
			{@attach contextMenu(
				() => albumItems(album),
				() => albumHeader(album)
			)}
		>
			<a
				href="/album/{album.id}"
				class="min-w-0 flex-1 self-stretch pl-2 {wide ? 'grid items-center gap-4' : 'flex items-center gap-3'}"
				style:grid-template-columns={wide ? listColumns(art) : undefined}
			>
				{#if art}
					<Cover art={album.art} class="size-10 shrink-0 rounded-md" />
				{/if}
				{#if wide}
					<span role="cell" class="flex min-w-0 items-center gap-2 text-sm font-medium {playing ? 'text-accent' : ''}">
						{#if playing}<Equalizer playing={hub.playing} />{/if}
						<span class="truncate">{album.title}</span>
						{@render versionsTag(album)}
					</span>
					<span role="cell" class="truncate text-sm text-muted">{album.artist}</span>
					<span role="cell" class="text-sm text-muted tabular-nums">{album.year ?? ''}</span>
					<span role="cell" class="truncate text-sm text-muted">{album.genre ?? ''}</span>
					<span role="cell" class="text-right text-sm text-muted tabular-nums">{album.tracks}</span>
					<span role="cell" class="text-right text-sm text-muted tabular-nums">{clock(album.duration)}</span>
					<span role="cell" class="text-right">{@render quality(album)}</span>
				{:else}
					<span role="cell" class="min-w-0 flex-1">
						<span class="flex items-center gap-2 text-[15px] leading-5 font-medium {playing ? 'text-accent' : ''}">
							{#if playing}<Equalizer playing={hub.playing} />{/if}
							<span class="truncate">{album.title}</span>
							{@render versionsTag(album)}
						</span>
						<span class="block truncate text-[13px] leading-5 text-muted">{album.artist}</span>
					</span>
					<span role="cell" class="flex shrink-0 flex-col items-end text-xs leading-5 text-muted tabular-nums">
						<span>{album.year ?? ''}</span>
						{@render quality(album)}
					</span>
				{/if}
			</a>
			<div role="cell" class="flex w-10 shrink-0 justify-center">
				<Menu items={() => albumItems(album)} header={albumHeader(album)} />
			</div>
		</div>
	{/each}
</div>
