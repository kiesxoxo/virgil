<script lang="ts">
	// Below the library's search results: the same words in Lidarr's catalogue, artists and albums, to
	// download what isn't here.
	import { ArrowDownToLine, LoaderCircle } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { api } from '../api.ts';
	import { find } from '../browse.ts';
	import { downloads } from '../downloads.svelte.ts';
	import { library } from '../library.svelte.ts';
	import type { CatalogAlbum } from '../types/CatalogAlbum.ts';
	import type { CatalogArtist } from '../types/CatalogArtist.ts';
	import type { CatalogResults } from '../types/CatalogResults.ts';
	import CatalogRow from './CatalogRow.svelte';

	let { query }: { query: string } = $props();

	let term = $derived(query.trim());
	let results = $state.raw<{ term: string; found: CatalogResults } | null>(null);
	let looking = $state(false);
	let shown = $derived(results?.term === term ? results.found : null);

	async function look() {
		const asked = term;
		looking = true;
		try {
			const found = await api.catalogSearch(asked);
			results = { term: asked, found };
		} finally {
			looking = false;
		}
	}

	function openAlbum(album: CatalogAlbum) {
		if (album.albumId && album.have >= album.total) goto(`/album/${album.albumId}`);
		else downloads.open(album);
	}

	/** An artist the library has goes to their page (which lists what's missing); others open in a sheet. */
	function openArtist(artist: CatalogArtist) {
		const here = find(library.artists, artist.name);
		if (here) goto(`/artist/${encodeURIComponent(here.name)}`);
		else downloads.openArtist(artist);
	}
</script>

{#snippet heading(text: string)}
	<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">{text}</h2>
{/snippet}

{#if downloads.on && term.length >= 2}
	<section class="border-b border-line">
		{#if shown}
			{#if shown.artists.length}
				{@render heading('Artists in Lidarr’s catalogue')}
				<ul class="grid sm:grid-cols-2 xl:grid-cols-3">
					{#each shown.artists.slice(0, 6) as artist (artist.id)}
						{@const here = !!find(library.artists, artist.name)}
						<li class="border-b border-line">
							<button type="button" class="press flex min-h-14 w-full items-center gap-3 px-3 py-2 text-left" onclick={() => openArtist(artist)}>
								<span class="min-w-0 flex-1">
									<span class="wide block truncate text-[13px] leading-[18px]">{artist.name}</span>
									{#if artist.disambiguation}<span class="block truncate text-xs leading-4 text-muted">{artist.disambiguation}</span>{/if}
								</span>
								{#if here}<span class="label shrink-0 text-[11px] text-muted">In library</span>{/if}
							</button>
						</li>
					{/each}
				</ul>
			{/if}
			{#if shown.albums.length}
				{@render heading('Albums in Lidarr’s catalogue')}
				<ul class="grid sm:grid-cols-2 xl:grid-cols-3">
					{#each shown.albums as album (album.id)}
						<li class="border-b border-line last:border-b-0"><CatalogRow {album} onclick={() => openAlbum(album)} /></li>
					{/each}
				</ul>
			{/if}
			{#if !shown.artists.length && !shown.albums.length}
				<p class="px-3 py-3 text-[13px] text-muted">Lidarr knows nothing by “{term}”.</p>
			{/if}
		{:else}
			<button type="button" class="press flex h-12 w-full items-center gap-3 px-3 text-left disabled:text-faint" disabled={looking} onclick={look}>
				{#if looking}<LoaderCircle class="size-4 shrink-0" />{:else}<ArrowDownToLine class="size-4 shrink-0" />{/if}
				<span class="wide min-w-0 truncate text-[13px]">Find “{term}” to download</span>
			</button>
		{/if}
	</section>
{/if}
