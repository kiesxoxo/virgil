<script lang="ts">
	import { LoaderCircle } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import Masthead from '#lib/components/Masthead.svelte';
	import { api } from '#lib/api.ts';
	import { hhmm } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { line } from '#lib/party.svelte.ts';
	import { toasts } from '#lib/toast.svelte.ts';
	import { radio } from '#lib/radio.svelte.ts';
	import type { Station } from '#lib/types/Station.ts';

	// NTS, laid out like the Atonemo NTS Radio Player: a button for each of
	// the two live channels and a dial of the sixteen Infinite Mixtapes.

	onMount(() => void radio.load());

	let current = $derived(hub.current && !hub.external ? radio.station(hub.current.uri) : undefined);
	/** The station just tapped, until the queue shows it playing. */
	let pending = $state<string | null>(null);

	$effect(() => {
		if (pending && current?.id === pending) pending = null;
	});

	async function tune(station: Station) {
		// A station plays at once, ahead of everyone's requests: the host's call.
		if (!line.host) return toasts.show('The host tunes the radio tonight', 'done');
		// Tapping what's playing stops it (radio stops rather than pauses).
		if (current?.id === station.id && hub.playing) return hub.toggle();
		pending = station.id;
		setTimeout(() => pending === station.id && (pending = null), 8000);
		try {
			await api.playStation(station.id);
		} catch {
			pending = null;
		}
	}

	const on = (station: Station) => current?.id === station.id;
</script>

<svelte:head><title>Radio · Virgil</title></svelte:head>

<div class="pt-safe">
	<Masthead word="RADIO" />

	{#if !radio.data}
		{#if radio.error}
			<div class="border-t border-line px-3 pt-8">
				<h2 class="wide text-2xl">NTS can't be reached</h2>
				<p class="mt-2 text-sm text-muted">Virgil needs internet access to play radio. {radio.error}</p>
				<button type="button" class="press wide mt-5 h-14 w-full bg-accent text-[15px] text-accent-fg uppercase" onclick={() => radio.load()}>Try again</button>
			</div>
		{:else}
			<div class="grid grid-cols-2 border-t border-line">
				<div class="aspect-square border-r border-line bg-surface"></div>
				<div class="aspect-square bg-surface"></div>
			</div>
		{/if}
	{:else}
		<div class="flex h-9 items-center justify-between border-y border-line px-3">
			<h2 class="label flex items-center gap-1.5 text-[11px] text-accent"><span class="size-1.5 rounded-full bg-accent"></span>Live</h2>
			<a href="https://www.nts.live" target="_blank" rel="noopener" class="label text-[11px] text-muted hover:text-fg">NTS</a>
		</div>
		<!-- Side by side even on phones, like the player's two buttons. -->
		<div class="grid grid-cols-2">
			{#each radio.data.live as station, i (station.id)}
				{@const show = station.now}
				{@const progress = radio.progress(station)}
				<div class="relative flex min-w-0 flex-col odd:rule-r">
					<button
						type="button"
						class="group relative block aspect-square w-full overflow-hidden bg-surface-2 text-left sm:aspect-[16/10]"
						aria-label="Play {station.name}{show ? `: ${show.title}` : ''}"
						onclick={() => tune(station)}
					>
						{#if station.art}
							<img src={station.art} alt="" class="absolute inset-0 size-full object-cover" />
						{/if}
						<span
							class="wide absolute top-0 left-0 grid size-[4.5rem] place-items-center text-[52px] leading-none sm:size-24 sm:text-[72px] {on(station)
								? 'bg-accent text-accent-fg'
								: 'bg-black text-[#f1efe8] group-active:bg-[#f1efe8] group-active:text-black'}"
						>
							{i + 1}
						</span>
						<span class="absolute top-0 right-0 flex h-8 items-center gap-2 bg-black px-2 text-[#f1efe8]">
							{#if pending === station.id}<LoaderCircle class="size-4" />{/if}
							<span class="label flex items-center gap-1.5 text-[10px]"><span class="size-1.5 rounded-full bg-accent"></span>Live</span>
						</span>
					</button>
					<div class="flex flex-1 flex-col border-t border-line px-2.5 pt-2.5 pb-3">
						{#if show}
							<div class="label truncate text-[10px] text-muted">{[show.location, `${hhmm(show.start)}–${hhmm(show.end)}`].filter(Boolean).join(' / ')}</div>
							<div class="wide mt-1.5 line-clamp-3 text-[13px] leading-[1.15] [overflow-wrap:anywhere] sm:text-lg">{show.title}</div>
							{#if progress !== null}
								<div class="mt-auto pt-2.5">
									<div class="h-2 overflow-hidden border border-accent">
										<div class="h-full bg-accent" style:transform="translateX({progress * 100 - 100}%)"></div>
									</div>
								</div>
							{/if}
						{:else}
							<div class="wide text-[15px]">{station.name}</div>
						{/if}
					</div>
					{#if station.next}
						<p class="truncate border-t border-line px-2.5 py-2 text-xs text-muted">
							<span class="label text-[10px] text-fg">Next {hhmm(station.next.start)}</span>&ensp;{station.next.title}
						</p>
					{/if}
				</div>
			{/each}
		</div>

		<h2 class="label flex h-9 items-center border-y border-line px-3 text-[11px] text-muted">Infinite Mixtapes</h2>
		<!-- A step sequencer's matrix: the keys butt up, and the one playing is lit. -->
		<ul class="grid grid-cols-4 sm:grid-cols-8">
			{#each radio.data.mixtapes as station, i (station.id)}
				{@const active = on(station)}
				<li class="relative border-b border-line {(i + 1) % 8 ? ((i + 1) % 4 ? 'rule-r' : 'sm:rule-r') : ''}">
					<button
						type="button"
						class="flex aspect-square w-full flex-col items-center justify-between px-1 pt-2.5 pb-1.5 text-center {active ? 'press bg-accent text-accent-fg' : 'press'}"
						title={station.tagline}
						aria-label="Play {station.name}"
						onclick={() => tune(station)}
					>
						<span class="relative grid min-h-0 w-full flex-1 place-items-center">
							{#if active}
								{#if station.iconBlack}<img src={station.iconBlack} alt="" class="size-9" />{/if}
							{:else}
								{#if station.iconBlack}<img src={station.iconBlack} alt="" class="size-9 dark:hidden" />{/if}
								{#if station.iconWhite}<img src={station.iconWhite} alt="" class="hidden size-9 dark:block" />{/if}
							{/if}
							{#if pending === station.id}<span class="absolute top-0 right-0.5"><LoaderCircle class="size-4" /></span>{/if}
						</span>
						<span class="label line-clamp-2 w-full text-[9px] leading-[11px]">{station.name}</span>
					</button>
				</li>
			{/each}
		</ul>

		<p class="label px-3 py-4 text-[10px] text-muted">
			Live radio and Infinite Mixtapes from <a href="https://www.nts.live" target="_blank" rel="noopener" class="underline hover:text-fg">NTS</a>
		</p>
	{/if}
</div>
