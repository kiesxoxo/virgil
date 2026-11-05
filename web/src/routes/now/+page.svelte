<script lang="ts">
	import { Airplay, ChevronDown, Disc3, ExternalLink, ListMusic } from '@lucide/svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import Cover from '#lib/components/Cover.svelte';
	import NowCover from '#lib/components/NowCover.svelte';
	import OnAir from '#lib/components/OnAir.svelte';
	import Scrubber from '#lib/components/Scrubber.svelte';
	import SignalBadge from '#lib/components/SignalBadge.svelte';
	import SignalPathCard from '#lib/components/SignalPathCard.svelte';
	import Transport from '#lib/components/Transport.svelte';
	import { api } from '#lib/api.ts';
	import { fitTitle } from '#lib/fit.ts';
	import { hhmm } from '#lib/format.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { isStream, radio } from '#lib/radio.svelte.ts';

	let item = $derived(hub.current);
	let shown = $derived(item ? radio.show(item) : null);
	let station = $derived(shown?.station ?? null);
	let stream = $derived(item ? isStream(item.uri) : false);
	let upNext = $derived(hub.currentIndex >= 0 ? hub.queue.items.slice(hub.currentIndex + 1, hub.currentIndex + 4) : []);
	let room = $state(0);
	let titleHeight = $state(0);
	let controlsHeight = $state(0);

	/** Opened by a link in the app, so going back stays in the app; unknown after a reload or a jump through history. */
	let fromApp = false;
	afterNavigate((navigation) => {
		// On the first load `from` is there too, with no route.
		if (navigation.type !== 'popstate') fromApp = !!navigation.from?.route.id;
	});

	function close() {
		if (fromApp) history.back();
		else goto('/');
	}
</script>

<svelte:head><title>{shown ? `${shown.title} · ${shown.subtitle}` : 'Now playing'} · Virgil</title></svelte:head>

<div class="pt-safe flex min-h-dvh flex-col">
	<header class="grid h-12 shrink-0 grid-cols-[56px_minmax(0,1fr)_56px] border-b border-line">
		<button type="button" class="press grid place-items-center" aria-label="Close" onclick={close}>
			<ChevronDown class="size-[22px]" />
		</button>
		<div class="grid min-w-0 place-items-center border-x border-line px-2">
			{#if hub.signal.verdict !== 'idle'}
				<SignalBadge signal={hub.signal} />
			{:else}
				<span class="label text-[10px] text-muted">Now playing</span>
			{/if}
		</div>
		<a href="/queue" class="press grid place-items-center" aria-label="Queue">
			<ListMusic class="size-5" />
		</a>
	</header>

	<div class="flex flex-1 flex-col md:grid md:grid-cols-[minmax(0,min(calc(100dvh-3rem),50vw))_minmax(0,1fr)] md:items-start">
		<div class="border-b border-line md:border-r md:border-b-0">
			<div class="mx-auto" class:fit={!hub.external} style:--below="{titleHeight + controlsHeight}px">
				{#if hub.external}
					<div class="grid aspect-square w-full place-items-center bg-surface text-info">
						<Airplay class="size-1/3" strokeWidth={1.25} />
					</div>
				{:else if item}
					<NowCover
						art={item.art}
						src={shown?.large ?? null}
						canPrevious={!stream && hub.currentIndex > 0}
						canNext={!stream && hub.currentIndex < hub.queue.items.length - 1}
						onclose={close}
					/>
				{:else}
					<div class="grid aspect-square w-full place-items-center bg-surface text-faint">
						<Disc3 class="size-1/3" strokeWidth={1} />
					</div>
				{/if}
			</div>
		</div>

		<div class="flex min-w-0 flex-1 flex-col self-stretch">
			{#if hub.external}
				<div class="px-3 pt-3.5 pb-4">
					<h1 class="wide text-[40px] leading-none">AirPlay</h1>
					<p class="mt-2 text-[15px] text-muted">Another device is streaming to this player. Its controls live on that device.</p>
				</div>
				<button type="button" class="press-lit wide h-14 bg-fg text-[15px] text-bg uppercase" onclick={() => api.play()}>Play from library here</button>
			{:else}
				<div class="px-3 pt-3.5 pb-3" bind:clientWidth={room} bind:clientHeight={titleHeight}>
					<h1 class="wide -mb-[0.16em] line-clamp-3 pb-[0.16em] leading-[1.03]" style:font-size="{room ? fitTitle(shown?.title ?? 'Not playing', room - 24, { scale: 1.1, max: 72, lines: 3 }) : 24}px">
						{shown?.title ?? 'Not playing'}
					</h1>
					{#if item?.albumId}
						<a href="/album/{item.albumId}" class="mt-2 block truncate text-[15px] text-muted hover:text-fg">
							{item.artist}{item.album ? ` — ${item.album}` : ''}
						</a>
					{:else if station}
						<a href="/radio" class="mt-2 block truncate text-[15px] text-muted hover:text-fg">{shown?.subtitle}</a>
					{:else}
						<p class="mt-2 truncate text-[15px] text-muted">{shown?.subtitle ?? 'Pick an album or a station'}</p>
					{/if}
				</div>
			{/if}

			<!-- Below on a phone, where the scrubber and the keys stay at the bottom of the screen. -->
			<div class="md:order-3">
				{#if station}
					{@const show = station.now}
					<div class="flex flex-col gap-3 border-t border-line px-3 py-3.5 text-sm">
						{#if show}
							{#if show.genres.length}
								<div class="flex flex-wrap gap-1.5">
									{#each show.genres as genre (genre)}
										<span class="label border border-line px-2 py-1 text-[10px]">{genre}</span>
									{/each}
								</div>
							{/if}
							{#if show.description}<p class="line-clamp-3 text-muted">{show.description}</p>{/if}
							{#if station.next}
								<p class="truncate text-muted">
									<span class="label text-[11px] text-fg">Next on {station.name} at {hhmm(station.next.start)}</span>&ensp;{station.next.title}
								</p>
							{/if}
							{#if show.url}
								<a href={show.url} target="_blank" rel="noopener" class="inline-flex w-fit items-center gap-1.5 font-medium text-muted hover:text-fg">
									This show on nts.live <ExternalLink class="size-3.5" />
								</a>
							{/if}
						{:else if station.description ?? station.tagline}
							<p class="line-clamp-3 text-muted">{station.description ?? station.tagline}</p>
						{/if}
					</div>
				{/if}

				<div class="border-t border-line">
					<SignalPathCard signal={hub.signal} details={false} />
				</div>

				{#if upNext.length}
					<div class="border-t border-line">
						<h2 class="label flex h-9 items-center px-3 text-[11px] text-muted">Up next</h2>
						<ul>
							{#each upNext as next (next.id)}
								{@const shownNext = radio.show(next)}
								<li class="border-t border-line">
									<button type="button" class="press flex w-full items-center gap-3 pr-3 text-left" onclick={() => api.playItem(next.id)}>
										<Cover art={shownNext.art} src={shownNext.src} class="size-14 shrink-0" />
										<span class="min-w-0">
											<span class="wide block truncate text-[13px] leading-[18px]">{shownNext.title}</span>
											<span class="block truncate text-xs leading-4 text-muted">{shownNext.subtitle}</span>
										</span>
									</button>
								</li>
							{/each}
						</ul>
					</div>
				{/if}
			</div>

			{#if !hub.external}
				<div class="pb-safe sticky bottom-0 mt-auto border-t border-line bg-bg md:static md:order-2 md:mt-0" bind:clientHeight={controlsHeight}>
					<div class="px-3 pt-1 pb-3">
						{#if stream}
							<OnAir {station} />
						{:else}
							<Scrubber value={hub.elapsed} max={hub.player.duration} disabled={!item} onseek={(s) => hub.seek(s)} />
						{/if}
					</div>
					<div class="border-t border-line">
						<Transport size="lg" />
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	/*
	 * On a phone the cover gives way when the screen is short, so the title and the controls both fit.
	 * It leaves the controls over the title's bottom padding, hiding the rule of what comes next.
	 */
	@media (max-width: 767.98px) {
		.fit {
			width: min(100%, max(60vw, calc(100dvh - env(safe-area-inset-top) - 3rem - 1px - var(--below))));
		}
	}
</style>
