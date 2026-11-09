<script lang="ts">
	// Under an album's tracklist: the songs Lidarr says it should have and the library doesn't.
	import { ArrowDownToLine } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { downloads } from '../downloads.svelte.ts';
	import { badge, describe, finished } from '../fetching.ts';
	import { clock, plural } from '../format.ts';
	import { hub } from '../hub.svelte.ts';
	import type { CatalogAlbum } from '../types/CatalogAlbum.ts';

	let { albumId }: { albumId: string } = $props();

	let lidarr = $state.raw<{ albumId: string; album: CatalogAlbum } | null>(null);

	$effect(() => {
		if (!downloads.on) return;
		const id = albumId;
		void hub.library.revision; // read again once a song is in
		api.missing(id).then(
			(album) => {
				if (albumId === id) lidarr = { albumId: id, album };
			},
			() => {
				if (albumId === id) lidarr = null;
			}
		);
	});

	let album = $derived(lidarr?.albumId === albumId ? lidarr.album : null);
	let missing = $derived(album?.tracks.filter((t) => !t.have) ?? []);
	let multiDisc = $derived(new Set(album?.tracks.map((t) => t.disc)).size > 1);
	let whole = $derived(album ? downloads.of(album.id) : undefined);
	let fetching = $derived(whole && !finished(whole) ? whole : null);
</script>

{#if album && missing.length}
	<section>
		<h2 class="label flex h-9 items-center justify-between border-b border-line bg-surface px-3 text-[11px] text-muted">
			<span>Missing</span><span>{plural(missing.length, 'song')} of {album.total}</span>
		</h2>
		<ol>
			{#each missing as track (track.id)}
				{@const song = downloads.of(album.id, track.id)}
				{@const busy = song && !finished(song)}
				<li class="flex min-h-[52px] items-stretch border-b border-line">
					<span class="label grid w-11 shrink-0 place-items-center text-[11px] text-faint">
						{multiDisc ? `${track.disc}·${track.number}` : String(track.number).padStart(2, '0')}
					</span>
					<span class="min-w-0 flex-1 self-center py-2">
						<span class="wide block text-[13px] leading-[17px] text-muted">{track.title}</span>
						{#if song}<span class="label block text-[10px] {busy ? 'text-accent' : 'text-muted'}">{describe(song)}</span>{/if}
					</span>
					<span class="label self-center px-3 text-[11px] text-faint">{clock(track.duration)}</span>
					{#if downloads.songs}
						{#if busy}
							<span class="label grid w-14 shrink-0 place-items-center border-l border-line text-[10px] text-accent">{badge(song)}</span>
						{:else}
							<button
								type="button"
								class="press grid w-14 shrink-0 place-items-center border-l border-line"
								title="Download {track.title}"
								aria-label="Download {track.title}"
								onclick={() => downloads.song(album.id, track.id)}
							>
								<ArrowDownToLine class="size-[18px]" />
							</button>
						{/if}
					{/if}
				</li>
			{/each}
		</ol>
		{#if fetching}
			<div class="border-b border-line px-3 py-2.5">
				<span class="label text-[11px] text-accent">{badge(fetching)}</span>
				<span class="ml-2 text-[13px] text-muted">{describe(fetching)}</span>
			</div>
		{:else}
			<button type="button" class="press label flex h-12 w-full items-center justify-center gap-2 border-b border-line text-[11px]" onclick={() => downloads.album(album.id)}>
				<ArrowDownToLine class="size-3.5" /> Download the whole album
			</button>
		{/if}
	</section>
{/if}
