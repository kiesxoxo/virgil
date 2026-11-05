<script lang="ts">
	import { ChevronLeft, ListChecks, ListEnd, ListPlus, Square, SquareCheck, Tags } from '@lucide/svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { afterNavigate, goto } from '$app/navigation';
	import Cover from '#lib/components/Cover.svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import Glyph from '#lib/components/Glyph.svelte';
	import Menu from '#lib/components/Menu.svelte';
	import { trackHeader, trackItems } from '#lib/actions.ts';
	import { api, queue } from '#lib/api.ts';
	import { contextMenu } from '#lib/menu.svelte.ts';
	import { pullBack } from '#lib/swipeback.ts';
	import { fitTitle } from '#lib/fit.ts';
	import { clock, formatLong, formatShort, isHiRes, span } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { library } from '#lib/library.svelte.ts';
	import { tagEditor } from '#lib/tageditor.svelte.ts';
	import type { AlbumDetail } from '#lib/types/AlbumDetail.ts';
	import type { Track } from '#lib/types/Track.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	/** The page this one was opened from, when it was a link in the app; unknown after a reload or a jump through history. */
	let from = $state<string | null>(null);
	afterNavigate((navigation) => {
		if (navigation.type === 'popstate') return;
		// Switching version replaces this page in history, so it was still opened from the same place.
		if (versions.some((version) => version.id === navigation.from?.params?.id)) return;
		from = navigation.from?.route.id ?? null;
	});

	/** Back to where the album was opened from, as it was left (the grid scrolled); else to the albums. */
	function back() {
		if (from) history.back();
		else goto('/');
	}

	let detail = $state<AlbumDetail | null>(null);
	let missing = $state(false);

	$effect(() => {
		const pending = data.detail;
		detail = null;
		missing = false;
		pending.then(
			(d) => {
				if (data.detail === pending) detail = d;
			},
			() => {
				if (data.detail === pending) missing = true;
			}
		);
	});

	// Refetch the tracks when the library changes (e.g. after a tag edit),
	// keeping the current list on screen until the new one arrives.
	let seenRevision = hub.library.revision;
	$effect(() => {
		const revision = hub.library.revision;
		if (revision === seenRevision) return;
		seenRevision = revision;
		const id = data.id;
		api.album(id).then(
			(d) => {
				if (data.id === id) detail = d;
			},
			() => {}
		);
	});

	let album = $derived(detail?.album ?? library.get(data.id));

	let versions = $derived(library.versions(data.id));
	let tracks = $derived(detail?.tracks ?? []);
	/** The tracks by disc; `start` is the index of the disc's first track in `tracks`. */
	type Side = { number: number; title: string | null; start: number; tracks: Track[]; duration: number };
	let discs = $derived.by(() => {
		const titles = new Map(detail?.discs.map((d) => [d.number, d.title]));
		const sides: Side[] = [];
		tracks.forEach((track, i) => {
			const number = track.disc ?? 1;
			let side = sides.at(-1);
			if (side?.number !== number) {
				side = { number, title: titles.get(number) ?? null, start: i, tracks: [], duration: 0 };
				sides.push(side);
			}
			side.tracks.push(track);
			side.duration += track.duration;
		});
		return sides;
	});
	let multiDisc = $derived(discs.length > 1);
	// Picking tracks to edit, like a selection in foobar2000's playlist.
	let selecting = $state(false);
	const selected = new SvelteSet<string>();
	let anchor = 0;

	$effect(() => {
		void data.id;
		selecting = false;
		selected.clear();
	});

	/** Click toggles; Shift-click selects the range from the last click. */
	function pick(i: number, range: boolean) {
		const uri = tracks[i].uri;
		if (range) {
			const [from, to] = anchor < i ? [anchor, i] : [i, anchor];
			for (const t of tracks.slice(from, to + 1)) selected.add(t.uri);
		} else if (selected.has(uri)) {
			selected.delete(uri);
		} else {
			selected.add(uri);
		}
		anchor = i;
	}

	function trackClick(e: MouseEvent, i: number) {
		if (!selecting && (e.shiftKey || e.metaKey || e.ctrlKey)) selecting = true;
		if (selecting) {
			e.preventDefault();
			pick(i, e.shiftKey);
		} else if (album) {
			queue.playAlbum(album.id, tracks[i].uri);
		}
	}

	/** Selects the whole disc, or clears it when it is all selected already. */
	function pickDisc(side: Side) {
		const all = side.tracks.every((t) => selected.has(t.uri));
		for (const t of side.tracks) {
			if (all) selected.delete(t.uri);
			else selected.add(t.uri);
		}
		anchor = side.start;
	}

	function stopSelecting() {
		selecting = false;
		selected.clear();
	}

	/** The selected tracks' URIs, in album order rather than click order. */
	const selectedUris = () => tracks.filter((t) => selected.has(t.uri)).map((t) => t.uri);

	function editSelected() {
		const uris = selectedUris();
		tagEditor.show(uris, uris.length === 1 ? (tracks.find((t) => t.uri === uris[0])?.title ?? '') : (album?.title ?? ''));
	}

	let meta = $derived(
		album
			? [
					album.year,
					album.discs > 1 && `${album.discs} discs`,
					`${album.tracks} ${album.tracks === 1 ? 'track' : 'tracks'}`,
					span(album.duration),
					album.format && formatShort(album.format)
				].filter(Boolean)
			: []
	);
	let room = $state(0);
</script>

<svelte:head><title>{album ? `${album.title} · ${album.artist}` : 'Album'} · Virgil</title></svelte:head>

{#if missing && !album}
	<div class="pt-safe px-3">
		<h1 class="wide mt-16 text-2xl">Album not found</h1>
		<p class="mt-2 text-sm text-muted">It may have been removed in the last rescan.</p>
		<a href="/" class="label mt-4 inline-block text-[11px] text-accent">Back to albums</a>
	</div>
{:else}
	<!-- clip, not hidden: it can't be scrolled sideways by focus, and the selection bar can stick. -->
	<div class="pt-safe relative isolate overflow-clip" {@attach pullBack}>
		<div class="flex h-11 items-stretch border-b border-line">
			<a
				href="/"
				data-back
				class="press label flex shrink-0 items-center gap-1 pr-4 pl-2 text-[11px]"
				onclick={(e) => {
					if (!from || e.metaKey || e.ctrlKey || e.shiftKey) return;
					e.preventDefault();
					back();
				}}
			>
				<ChevronLeft class="size-4" /> {from && from !== '/' ? 'Back' : 'Albums'}
			</a>
			<span class="label ml-auto flex min-w-0 items-center px-3 text-[11px] text-muted"><span class="truncate">{album?.genre ?? ''}</span></span>
		</div>

		<div class="md:grid md:grid-cols-[minmax(0,26rem)_minmax(0,1fr)] lg:grid-cols-[minmax(0,32rem)_minmax(0,1fr)]">
			<header class="top-safe md:sticky md:self-start md:border-r md:border-line">
				<!-- The cover goes back. -->
				<button type="button" class="block w-full cursor-pointer" aria-label="Back" title="Back" onclick={back}>
					<Cover art={album?.art} size={1000} eager class="w-full" />
				</button>
				<div class="border-t border-line px-3 pt-3 pb-3" bind:clientWidth={room}>
					<h1 class="wide leading-[1.03] text-balance" style:font-size="{room && album ? fitTitle(album.title, room - 24, { scale: 1.1, max: 56 }) : 32}px">{album?.title ?? ''}</h1>
					<p class="mt-2 text-base font-semibold">
						{#if album}<a href="/artist/{encodeURIComponent(album.artist)}" class="hover:underline">{album.artist}</a>{/if}
					</p>
					<p class="label mt-1.5 flex flex-wrap items-center gap-x-2 text-[10.5px] leading-4 text-muted" title={album?.format ? formatLong(album.format) : undefined}>
						{meta.join(' / ')}
						{#if album?.format && isHiRes(album.format)}<span class="bg-fg px-1 text-bg">Hi-res</span>{/if}
					</p>
				</div>
				{#if versions.length > 1}
					<div class="flex flex-wrap border-t border-line" role="radiogroup" aria-label="Versions">
						{#each versions as version (version.id)}
							{@const active = version.id === data.id}
							<a
								href="/album/{version.id}"
								data-sveltekit-replacestate
								role="radio"
								aria-checked={active}
								title={version.title}
								class="label flex h-11 flex-1 items-center justify-center gap-1 border-r border-line px-3 text-[10.5px] whitespace-nowrap last:border-r-0 {active
									? 'bg-fg text-bg'
									: 'press text-muted'}"
							>
								{version.format ? formatShort(version.format) : 'Mixed'} / {version.tracks} tr
							</a>
						{/each}
					</div>
				{/if}
				<!-- On a phone the keys wait above the player while the cover and title take the screen. -->
				<div class="grid h-14 grid-cols-[minmax(0,1fr)_repeat(3,56px)] border-t border-line bg-bg max-md:sticky max-md:bottom-[calc(7.5rem+env(safe-area-inset-bottom))]">
					<button
						type="button"
						class="press wide flex items-center justify-center gap-2.5 bg-accent text-[17px] text-accent-fg disabled:bg-surface-2 disabled:text-faint"
						disabled={!album}
						onclick={() => album && queue.playAlbum(album.id)}
					>
						<Glyph name="play" class="size-4" /> PLAY
					</button>
					<button
						type="button"
						class="press grid place-items-center border-l border-line disabled:text-faint"
						disabled={!album}
						aria-label="Play next"
						title="Play next"
						onclick={() => album && queue.albumNext(album.id)}
					>
						<ListPlus class="size-5" />
					</button>
					<button
						type="button"
						class="press grid place-items-center border-l border-line disabled:text-faint"
						disabled={!album}
						aria-label="Add to queue"
						title="Add to queue"
						onclick={() => album && queue.albumLast(album.id)}
					>
						<ListEnd class="size-5" />
					</button>
					<button
						type="button"
						class="grid place-items-center border-l border-line disabled:text-faint {selecting ? 'press-lit bg-fg text-bg' : 'press'}"
						disabled={!detail}
						aria-pressed={selecting}
						aria-label="Select tracks"
						title="Select tracks to queue or edit"
						onclick={() => (selecting ? stopSelecting() : (selecting = true))}
					>
						<ListChecks class="size-5" />
					</button>
				</div>
			</header>

			<div class="border-t border-line md:border-t-0">
				{#if selecting}
					<div class="top-safe sticky z-10 flex h-11 items-stretch border-b border-line bg-bg">
						<span class="label flex flex-1 items-center px-3 text-[11px] whitespace-nowrap">{selected.size} selected</span>
						<button
							type="button"
							class="press label border-l border-line px-3 text-[11px]"
							onclick={() => (selected.size === tracks.length ? selected.clear() : tracks.forEach((t) => selected.add(t.uri)))}
						>
							{selected.size === tracks.length ? 'None' : 'All'}
						</button>
						<button
							type="button"
							class="press grid w-11 place-items-center border-l border-line disabled:text-faint"
							disabled={!selected.size}
							title="Play next"
							aria-label="Play next"
							onclick={() => queue.tracksNext(selectedUris())}
						>
							<ListPlus class="size-[18px]" />
						</button>
						<button
							type="button"
							class="press grid w-11 place-items-center border-l border-line disabled:text-faint"
							disabled={!selected.size}
							title="Add to end of queue"
							aria-label="Add to end of queue"
							onclick={() => queue.tracksLast(selectedUris())}
						>
							<ListEnd class="size-[18px]" />
						</button>
						<button
							type="button"
							class="press grid w-11 place-items-center border-l border-line bg-accent text-accent-fg disabled:bg-surface-2 disabled:text-faint"
							disabled={!selected.size}
							title="Edit tags"
							aria-label="Edit tags"
							onclick={editSelected}
						>
							<Tags class="size-[18px]" />
						</button>
						<button type="button" class="press label border-l border-line px-3 text-[11px]" onclick={stopSelecting}>Done</button>
					</div>
				{/if}

				<ol>
					{#if !detail}
						{#each { length: Math.min(album?.tracks ?? 8, 12) } as _, i (i)}
							<li class="h-[52px] border-b border-line"></li>
						{/each}
					{/if}
					{#each discs as side (side.number)}
						{#if multiDisc}
							{@render discHeader(side)}
						{/if}
						{#each side.tracks as track, j (track.uri)}
							{@render trackRow(track, side.start + j)}
						{/each}
					{/each}
				</ol>
			</div>
		</div>
	</div>
{/if}

{#snippet discHeader(side: Side)}
	{@const onDisc = side.tracks.some((t) => t.uri === hub.current?.uri)}
	{@const label = `Disc ${side.number}`}
	{@const allPicked = side.tracks.every((t) => selected.has(t.uri))}
	<li class="flex min-h-11 items-stretch border-b border-line bg-surface">
		<div class="min-w-0 flex-1 self-center px-3 py-1.5">
			<h2 class="label truncate text-[11px] {onDisc ? 'text-accent' : ''}" title={side.title ?? undefined}>{side.title ?? label}</h2>
			<p class="label truncate text-[10px] text-muted">
				{[side.title && label, `${side.tracks.length} ${side.tracks.length === 1 ? 'track' : 'tracks'}`, span(side.duration)]
					.filter(Boolean)
					.join(' / ')}
			</p>
		</div>
		{#if selecting}
			<button type="button" class="press grid w-11 shrink-0 place-items-center border-l border-line" aria-pressed={allPicked} title="Select {label}" onclick={() => pickDisc(side)}>
				{#if allPicked}<SquareCheck class="size-5 text-accent" />{:else}<Square class="size-5 text-muted" />{/if}
			</button>
		{:else}
			<button
				type="button"
				class="press grid w-11 shrink-0 place-items-center border-l border-line"
				disabled={!album}
				title="Play {label}"
				aria-label="Play {label}"
				onclick={() => album && queue.playAlbum(album.id, side.tracks[0].uri)}
			>
				<Glyph name="play" class="size-4" />
			</button>
		{/if}
	</li>
{/snippet}

{#snippet trackRow(track: Track, i: number)}
	{@const current = hub.current?.uri === track.uri}
	{@const picked = selecting && selected.has(track.uri)}
	<li
		class="group flex items-stretch border-b border-line hover:bg-surface {picked ? 'bg-surface-2' : ''}"
		{@attach contextMenu(
			() => trackItems(track, data.id),
			() => trackHeader(track, album?.art)
		)}
	>
		<button
			type="button"
			class="press grid min-h-[52px] min-w-0 flex-1 grid-cols-[2.75rem_minmax(0,1fr)_auto] items-center py-2 text-left select-none"
			aria-pressed={selecting ? picked : undefined}
			onclick={(e) => trackClick(e, i)}
		>
			<span class="label flex justify-center text-[11px] text-muted">
				{#if selecting}
					{#if picked}<SquareCheck class="size-5 text-accent" />{:else}<Square class="size-5 text-muted" />{/if}
				{:else if current}<Equalizer playing={hub.playing} class="text-accent" />{:else if track.track !== null}{String(track.track).padStart(2, '0')}{/if}
				<!-- No track number tag: blank rather than a position, so the gap shows. -->
			</span>
			<span class="min-w-0">
				<span class="wide block text-[13px] leading-[17px] {current ? 'text-accent' : ''}">{track.title}</span>
				{#if album && track.artist && track.artist !== album.artist}
					<span class="mt-0.5 block truncate text-xs text-muted">{track.artist}</span>
				{/if}
			</span>
			<span class="label flex items-center gap-3 pl-3 text-[11px] text-muted">
				{#if track.format && !album?.format}
					<span class="max-sm:hidden">{formatShort(track.format)}</span>
				{/if}
				{clock(track.duration)}
			</span>
		</button>
		<Menu items={() => trackItems(track, data.id)} header={trackHeader(track, album?.art)} />
	</li>
{/snippet}
