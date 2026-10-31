<script lang="ts">
	import { LoaderCircle, RadioTower } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import Equalizer from '#lib/components/Equalizer.svelte';
	import { api } from '#lib/api.ts';
	import { hhmm } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
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

{#snippet status(station: Station)}
	{#if pending === station.id}
		<LoaderCircle class="size-4 animate-spin" />
	{:else if on(station)}
		<!-- On a dark card or an accent tile alike. -->
		<Equalizer playing={hub.playing} class="text-white!" />
	{/if}
{/snippet}

<div class="pt-safe mx-auto max-w-[110rem] px-4 pt-4 lg:px-8 lg:pt-8">
	<div class="flex items-baseline justify-between gap-3">
		<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">Radio</h1>
		<a href="https://www.nts.live" target="_blank" rel="noopener" class="text-sm font-semibold text-muted hover:text-fg">NTS</a>
	</div>

	{#if !radio.data}
		{#if radio.error}
			<div class="mx-auto mt-16 max-w-sm text-center">
				<div class="mx-auto grid size-16 place-items-center rounded-2xl bg-surface text-muted"><RadioTower class="size-7" /></div>
				<h2 class="mt-5 text-lg font-semibold">NTS can't be reached</h2>
				<p class="mt-1.5 text-sm text-muted">Virgil needs internet access to play radio. {radio.error}</p>
				<button type="button" class="press mt-5 h-10 rounded-full bg-accent px-5 text-sm font-semibold text-accent-fg" onclick={() => radio.load()}>
					Try again
				</button>
			</div>
		{:else}
			<div class="mt-6 grid animate-pulse grid-cols-2 gap-3 sm:gap-4">
				<div class="aspect-square rounded-3xl bg-surface sm:aspect-[16/10]"></div>
				<div class="aspect-square rounded-3xl bg-surface sm:aspect-[16/10]"></div>
			</div>
		{/if}
	{:else}
		<h2 class="mt-6 mb-3 text-xs font-semibold tracking-wider text-muted uppercase">Live</h2>
		<!-- Side by side even on phones, like the player's two buttons. -->
		<div class="grid grid-cols-2 gap-x-3 gap-y-5 sm:gap-x-4">
			{#each radio.data.live as station, i (station.id)}
				{@const show = station.now}
				{@const progress = radio.progress(station)}
				<div class="min-w-0">
					<button
						type="button"
						class="press group relative block aspect-square w-full overflow-hidden rounded-3xl sm:aspect-[16/10] bg-surface-2 text-left shadow-cover {on(station)
							? 'ring-2 ring-accent'
							: 'ring-1 ring-line'}"
						aria-label="Play {station.name}{show ? `: ${show.title}` : ''}"
						onclick={() => tune(station)}
					>
						{#if station.art}
							<img src={station.art} alt="" class="absolute inset-0 size-full object-cover transition-transform duration-700 group-hover:scale-[1.03]" />
						{/if}
						<div class="absolute inset-0 bg-gradient-to-t from-black/85 via-black/35 to-black/10"></div>
						<div class="relative flex h-full flex-col justify-between p-3 text-white sm:p-4 lg:p-5">
							<div class="flex items-center justify-between">
								<span class="grid size-9 place-items-center rounded-full bg-white text-lg font-black text-black shadow-float sm:size-11 sm:text-xl">{i + 1}</span>
								<span class="flex items-center gap-2">
									{@render status(station)}
									<span class="inline-flex items-center gap-1.5 rounded-full bg-black/55 px-2 py-1 text-[10px] font-bold tracking-wider sm:px-2.5 sm:text-[11px]">
										<span class="size-1.5 animate-pulse rounded-full bg-accent"></span>LIVE
									</span>
								</span>
							</div>
							<div class="min-w-0">
								{#if show}
									<div class="truncate text-[10px] font-semibold tracking-wider text-white/75 uppercase sm:text-xs">
										{[show.location, `${hhmm(show.start)}–${hhmm(show.end)}`].filter(Boolean).join(' · ')}
									</div>
									<div class="mt-1 line-clamp-3 text-[15px] leading-tight font-bold sm:line-clamp-2 sm:text-lg sm:leading-snug lg:text-xl">{show.title}</div>
									{#if progress !== null}
										<div class="mt-2.5 h-1 overflow-hidden rounded-full bg-white/25 sm:mt-3">
											<div class="h-full rounded-full bg-white transition-transform duration-1000" style:transform="translateX({progress * 100 - 100}%)"></div>
										</div>
									{/if}
								{:else}
									<div class="text-lg font-bold">{station.name}</div>
								{/if}
							</div>
						</div>
					</button>
					{#if station.next}
						<p class="mt-2 truncate px-1 text-[13px] text-muted">
							<span class="font-semibold text-fg">Next {hhmm(station.next.start)}</span>&ensp;{station.next.title}
						</p>
					{/if}
				</div>
			{/each}
		</div>

		<h2 class="mt-9 mb-3 text-xs font-semibold tracking-wider text-muted uppercase">Infinite Mixtapes</h2>
		<ul class="grid grid-cols-4 gap-x-3 gap-y-5 sm:grid-cols-8 lg:gap-x-5">
			{#each radio.data.mixtapes as station (station.id)}
				{@const active = on(station)}
				<li>
					<button
						type="button"
						class="press group flex w-full flex-col items-center gap-2 text-center"
						title={station.tagline}
						aria-label="Play {station.name}"
						onclick={() => tune(station)}
					>
						<span
							class="relative grid aspect-square w-full place-items-center rounded-2xl ring-1 ring-line transition-colors {active
								? 'bg-accent text-accent-fg'
								: 'bg-surface group-hover:bg-surface-2'}"
						>
							{#if active}
								{#if station.iconWhite}<img src={station.iconWhite} alt="" class="size-1/2" />{/if}
							{:else}
								{#if station.iconBlack}<img src={station.iconBlack} alt="" class="size-1/2 dark:hidden" />{/if}
								{#if station.iconWhite}<img src={station.iconWhite} alt="" class="hidden size-1/2 dark:block" />{/if}
							{/if}
							<span class="absolute top-1.5 right-1.5">{@render status(station)}</span>
						</span>
						<span class="w-full truncate text-[12px] leading-4 font-semibold sm:text-[13px]">{station.name}</span>
					</button>
				</li>
			{/each}
		</ul>

		<p class="mt-10 text-xs text-faint">
			Live radio and Infinite Mixtapes from <a href="https://www.nts.live" target="_blank" rel="noopener" class="underline hover:text-muted">NTS</a>.
		</p>
	{/if}
</div>
