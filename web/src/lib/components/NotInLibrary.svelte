<script lang="ts">
	// On an artist's page: the albums Lidarr knows by them that the library has none of.
	import { ArrowDownToLine, LoaderCircle } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { downloads } from '../downloads.svelte.ts';
	import { fold } from '../format.ts';
	import { hub } from '../hub.svelte.ts';
	import { toasts } from '../toast.svelte.ts';
	import type { CatalogAlbum } from '../types/CatalogAlbum.ts';
	import CatalogRow from './CatalogRow.svelte';

	let { name }: { name: string } = $props();

	let catalog = $state.raw<{ name: string; known: boolean; albums: CatalogAlbum[] } | null>(null);
	let looking = $state(false);

	$effect(() => {
		if (!downloads.on) return;
		const asked = name;
		void hub.library.revision; // read again once a download is in
		api.catalogArtist(asked).then(
			(c) => {
				if (name === asked) catalog = { name: asked, ...c };
			},
			() => {}
		);
	});

	/** Lidarr doesn't follow the artist: find them in its catalogue, and open them in the artist sheet. */
	async function lookUp() {
		const asked = name;
		looking = true;
		try {
			const { artists } = await api.catalogSearch(asked);
			const namesakes = artists.filter((a) => fold(a.name) === fold(asked));
			const artist = namesakes.find((a) => a.known) ?? namesakes[0];
			if (artist && name === asked) downloads.openArtist(artist);
			else if (!artist) toasts.show(`Lidarr knows no artist called ${asked}`);
		} finally {
			looking = false;
		}
	}

	let albums = $derived(catalog?.name === name ? catalog.albums : []);
</script>

{#if downloads.on && catalog?.name === name}
	{#if albums.length}
		<section>
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">Not in library</h2>
			<ul class="grid sm:grid-cols-2 xl:grid-cols-3">
				{#each albums as album (album.id)}
					<li class="border-b border-line"><CatalogRow {album} onclick={() => downloads.open(album)} /></li>
				{/each}
			</ul>
		</section>
	{:else if !catalog.known}
		<button type="button" class="press flex h-12 w-full items-center gap-3 border-b border-line px-3 text-left disabled:text-faint" disabled={looking} onclick={lookUp}>
			{#if looking}<LoaderCircle class="size-4 shrink-0" />{:else}<ArrowDownToLine class="size-4 shrink-0" />{/if}
			<span class="wide min-w-0 truncate text-[13px]">Find more by {name} to download</span>
		</button>
	{/if}
{/if}
