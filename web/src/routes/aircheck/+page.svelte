<script lang="ts">
	// The night on the request line: its awards and its set list, to keep or send round.
	import { Copy, Heart } from '@lucide/svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Masthead from '#lib/components/Masthead.svelte';
	import Sign from '#lib/components/Sign.svelte';
	import { awards, setList, setListText, summary } from '#lib/aircheck.ts';
	import { api, ApiError } from '#lib/api.ts';
	import { hhmm } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { toasts } from '#lib/toast.svelte.ts';
	import type { Aircheck } from '#lib/types/Aircheck.ts';

	let night = $state.raw<Aircheck | null>(null);
	let none = $state(false);

	// Again whenever a song starts or votes change, so a night in progress keeps up.
	$effect(() => {
		void hub.current?.id;
		void hub.party.votes;
		void hub.party.open;
		api
			.aircheck()
			.then((n) => (night = n))
			.catch((e) => (none = e instanceof ApiError && e.status === 404));
	});

	let entries = $derived(night ? setList(night).toReversed() : []);
	let won = $derived(night ? awards(night) : []);
	let date = $derived(night ? new Date(night.opened * 1000).toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }) : '');

	/** The clipboard API needs HTTPS; http://virgil.local copies the old way. */
	async function copy() {
		if (!night) return;
		const text = setListText(night, hhmm);
		try {
			await navigator.clipboard.writeText(text);
		} catch {
			const area = document.createElement('textarea');
			area.value = text;
			area.setAttribute('readonly', '');
			area.style.position = 'fixed';
			area.style.opacity = '0';
			document.body.append(area);
			area.select();
			area.setSelectionRange(0, text.length);
			const ok = document.execCommand('copy');
			area.remove();
			if (!ok) return toasts.show('Couldn’t copy the set list');
		}
		toasts.done('Set list copied');
	}
</script>

<svelte:head><title>Aircheck · Virgil</title></svelte:head>

<div class="pt-safe lg:max-w-3xl">
	<Masthead word="AIRCHECK" />

	{#if !night}
		<div class="border-t border-line px-3 pt-8">
			{#if none}
				<h2 class="wide text-2xl">No nights yet</h2>
				<p class="mt-2 text-sm text-muted">Open the request line from the Queue or Settings: what plays while it’s open lands here, with who asked for it.</p>
			{/if}
		</div>
	{:else}
		<div class="flex items-stretch border-y border-line">
			<div class="min-w-0 flex-1 px-3 py-2.5">
				<p class="label text-[10.5px] {night.closed ? 'text-muted' : 'text-accent'}">
					{#if !night.closed}<span class="mr-1 inline-block size-1.5 rounded-full bg-current align-middle"></span>{'On air · '}{/if}{date}
				</p>
				<p class="mt-0.5 text-sm font-medium">{hhmm(night.opened)}{night.closed ? `–${hhmm(night.closed)}` : ''} · {summary(night)}</p>
			</div>
			<button type="button" class="press label flex shrink-0 items-center gap-2 border-l border-line px-3 text-[10.5px] disabled:text-faint" disabled={!night.played.length} onclick={copy}>
				<Copy class="size-3.5" /> Copy set list
			</button>
		</div>

		{#if won.length}
			<h2 class="label flex h-9 items-center border-b border-line bg-surface px-3 text-[11px] text-muted">Awards</h2>
			<ul class="grid grid-cols-2 border-b border-line md:grid-cols-3">
				{#each won as award (award.title)}
					<li class="-mb-px border-r border-b border-line px-3 py-3 [&:nth-child(2n)]:border-r-0 md:[&:nth-child(2n)]:border-r md:[&:nth-child(3n)]:border-r-0">
						<p class="label text-[10px] text-accent">{award.title}</p>
						<div class="mt-2 flex items-center gap-2">
							<Sign person={award.person} size="md" />
							<span class="wide min-w-0 truncate text-[15px]">{award.person?.name ?? 'Someone'}</span>
						</div>
						<p class="mt-1.5 line-clamp-2 text-xs text-muted">{award.detail}</p>
					</li>
				{/each}
			</ul>
		{/if}

		<h2 class="label flex h-9 items-center border-b border-line bg-surface px-3 text-[11px] text-muted">Set list</h2>
		{#if !entries.length}
			<p class="border-b border-line px-3 py-3 text-sm text-muted">Nothing has played yet. The first song on air starts the list.</p>
		{/if}
		<ol>
			{#each entries as entry (entry.songs[0].at + entry.songs[0].uri)}
				{@const first = entry.songs[0]}
				{@const cheers = entry.songs.reduce((n, s) => n + s.cheers, 0)}
				<li class="flex items-center gap-3 border-b border-line pr-3 [contain-intrinsic-size:auto_56px] [content-visibility:auto]">
					<Cover art={first.art} class="size-14 shrink-0" />
					<span class="min-w-0 flex-1 py-1.5">
						<span class="wide block truncate text-[13px] leading-[18px]">{entry.album ? first.album : entry.songs.map((s) => s.title).join(' / ')}</span>
						<span class="block truncate text-xs leading-4 text-muted">
							<span class="label text-[10px]">{hhmm(first.at)}</span>&ensp;{first.artist}{entry.album ? ` · ${entry.songs.length} songs` : ''}
						</span>
						{#if first.pick?.note}<span class="block truncate text-xs leading-4">“{first.pick.note}”</span>{/if}
					</span>
					{#if entry.songs.some((s) => s.skipped)}<span class="label border border-line px-1 text-[9px] text-muted">Skipped</span>{/if}
					{#if cheers}<span class="label flex items-center gap-1 text-[11px] text-accent"><Heart class="size-3.5" fill="currentColor" />{cheers}</span>{/if}
					{#if entry.person}<Sign person={entry.person} />{:else}<span class="label text-[9px] text-faint">House</span>{/if}
				</li>
			{/each}
		</ol>
	{/if}
</div>
