<script lang="ts">
	// An artist from Lidarr's catalogue and their albums; each opens in the catalogue sheet to download.
	import { LoaderCircle } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { downloads } from '../downloads.svelte.ts';
	import type { CatalogAlbum } from '../types/CatalogAlbum.ts';
	import CatalogRow from './CatalogRow.svelte';
	import Sheet from './Sheet.svelte';

	let artist = $derived(downloads.artist);
	let albums = $state.raw<{ id: string; list: CatalogAlbum[] } | null>(null);
	let following = $state(false);
	let shown = $derived(artist && albums?.id === artist.id ? albums.list : null);

	// Lidarr lists the albums of artists it follows.
	$effect(() => {
		const id = artist?.id;
		if (!id || !artist?.known) return;
		api.catalogArtistAlbums(id).then(
			(c) => {
				if (downloads.artist?.id === id) albums = { id, list: c.albums };
			},
			() => {}
		);
	});

	/** Has Lidarr follow the artist (wanting nothing), then asks until it has read their albums. */
	async function follow() {
		if (!artist) return;
		const id = artist.id;
		following = true;
		try {
			for (let tries = 0; tries < 20; tries++) {
				const c = await (tries === 0 ? api.followArtist(id) : api.catalogArtistAlbums(id));
				if (downloads.artist?.id !== id) return;
				if (c.albums.length) {
					albums = { id, list: c.albums };
					downloads.artist = { ...downloads.artist, known: true };
					return;
				}
				await new Promise((r) => setTimeout(r, 2500));
			}
			albums = { id, list: [] };
		} finally {
			following = false;
		}
	}
</script>

{#if artist}
	<Sheet onclose={() => downloads.closeArtist()} label={artist.name}>
		<div class="border-b border-line px-3 pt-4 pb-3">
			<p class="label text-[11px] text-muted">Artist{artist.disambiguation ? ` · ${artist.disambiguation}` : ''}</p>
			<h2 class="wide mt-1 text-[24px] leading-tight text-balance">{artist.name}</h2>
		</div>
		{#if shown}
			{#if shown.length}
				<ul>
					{#each shown as album (album.id)}
						<li class="border-b border-line"><CatalogRow {album} onclick={() => downloads.open(album)} /></li>
					{/each}
				</ul>
			{:else}
				<p class="border-b border-line px-3 py-3 text-[13px] text-muted">Lidarr lists no albums for {artist.name}.</p>
			{/if}
		{:else if artist.known}
			<p class="label flex h-12 items-center gap-2 border-b border-line px-3 text-[11px] text-muted"><LoaderCircle class="size-3.5" /> Reading their albums</p>
		{:else}
			<p class="border-b border-line px-3 pt-3 text-[13px] text-muted">Lidarr doesn’t follow {artist.name} yet. It lists their albums once it does; it won’t download anything you don’t ask for.</p>
			<button
				type="button"
				class="press wide mt-3 flex h-14 w-full items-center justify-center gap-2.5 border-y border-line bg-accent text-[15px] text-accent-fg uppercase disabled:bg-surface-2 disabled:text-faint"
				disabled={following}
				onclick={follow}
			>
				{#if following}<LoaderCircle class="size-4" /> Lidarr is reading them{:else}Show their albums{/if}
			</button>
		{/if}
		<button type="button" class="press label h-11 w-full text-[11px] text-muted" onclick={() => downloads.closeArtist()}>Close</button>
	</Sheet>
{/if}
