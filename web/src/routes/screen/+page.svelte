<script lang="ts">
	// The big screen: an iPad on a stand (or a TV) showing what's on, who asked for it, what's
	// next, and the code to scan to call in. It keeps the screen awake while it is shown.
	import { Heart, X } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import Cover from '#lib/components/Cover.svelte';
	import Logotype from '#lib/components/Logotype.svelte';
	import QrCode from '#lib/components/QrCode.svelte';
	import Sign from '#lib/components/Sign.svelte';
	import { goBack } from '#lib/back.ts';
	import { fitTitle } from '#lib/fit.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { line } from '#lib/party.svelte.ts';
	import { radio } from '#lib/radio.svelte.ts';
	import { blocks } from '#lib/requests.ts';

	let item = $derived(hub.current);
	let shown = $derived(item ? radio.show(item) : null);
	let picker = $derived(line.person(item?.pick));
	let next = $derived(hub.currentIndex < 0 ? [] : blocks(hub.queue.items.slice(hub.currentIndex + 1)).slice(0, 4));
	let votes = $derived(hub.party.votes);
	let room = $state(0);
	// The cover is the largest square that fits: beside the text on a landscape screen, above it otherwise.
	let wide = $state(0);
	let tall = $state(0);
	let landscape = $state(true);
	let side = $derived(landscape ? Math.min(tall, wide * 0.5) : Math.min(wide, tall * 0.45));
	const url = typeof location === 'undefined' ? '' : location.origin;

	onMount(() => {
		landscape = innerWidth > innerHeight;
		// A stand-up screen shouldn't dim and lock (iOS 16.4+; ignored where unsupported).
		let lock: WakeLockSentinel | null = null;
		const keep = async () => {
			if (document.visibilityState !== 'visible' || !('wakeLock' in navigator)) return;
			lock = await navigator.wakeLock.request('screen').catch(() => null);
		};
		void keep();
		document.addEventListener('visibilitychange', keep);
		return () => {
			document.removeEventListener('visibilitychange', keep);
			void lock?.release();
		};
	});
</script>

<svelte:window onresize={() => (landscape = innerWidth > innerHeight)} />
<svelte:head><title>Big screen · Virgil</title></svelte:head>

<div class="pt-safe pb-safe flex h-dvh flex-col overflow-hidden">
	<header class="flex h-14 shrink-0 items-stretch border-b border-line">
		<button type="button" class="press grid w-14 place-items-center border-r border-line" aria-label="Close" onclick={() => goBack('/queue')}>
			<X class="size-5" />
		</button>
		<div class="flex min-w-0 flex-1 items-center px-3"><Logotype class="h-6 w-auto" /></div>
		<p class="label flex items-center gap-2 border-l border-line px-3 text-[12px] {line.open ? 'text-accent' : 'text-muted'}">
			<span class="size-2 rounded-full bg-current"></span>
			{line.open ? `Line open · ${line.here.length} here` : 'Line closed'}
		</p>
	</header>

	<div class="flex min-h-0 flex-1 max-md:flex-col">
		<section class="flex min-h-0 min-w-0 flex-1 {landscape ? '' : 'flex-col'}" bind:clientWidth={wide} bind:clientHeight={tall}>
			<div class="shrink-0 border-line {landscape ? 'border-r' : 'mx-auto border-b'}" style:width="{side}px" style:height="{side}px">
				<Cover art={item?.art} src={shown?.large ?? null} size={1000} eager class="size-full" />
			</div>
			<div class="flex min-w-0 flex-1 flex-col overflow-hidden" bind:clientWidth={room}>
				<div class="px-5 pt-5">
					<p class="label text-[12px] text-muted">{hub.playing ? 'Now playing' : 'Paused'}</p>
					<h1 class="wide mt-1 line-clamp-3 leading-[1.03]" style:font-size="{room ? fitTitle(shown?.title ?? 'Nothing on', room - 40, { scale: 1.1, max: 80, lines: 3 }) : 32}px">
						{shown?.title ?? 'Nothing on'}
					</h1>
					<p class="mt-2 truncate text-[20px] text-muted">{shown?.subtitle ?? 'Request something: the line is open'}</p>
				</div>
				{#if item?.pick?.note}
					<p class="wide line-clamp-3 px-5 pt-4 text-[clamp(20px,2.4vw,36px)] leading-[1.08] [overflow-wrap:anywhere]">“{item.pick.note}”</p>
				{/if}
				<div class="mt-auto flex items-center gap-4 border-t border-line px-5 py-4">
					{#if picker}
						<Sign person={picker} size="xl" />
						<div class="min-w-0">
							<p class="label text-[12px] text-muted">Requested by</p>
							<p class="wide truncate text-[28px] leading-tight">{picker.name}</p>
						</div>
					{:else if item}
						<p class="label text-[13px] text-muted">On the house</p>
					{/if}
					{#if line.open && item}
						<div class="ml-auto flex shrink-0 items-center gap-5">
							<span class="label flex items-center gap-2 text-[16px] text-accent"><Heart class="size-5" fill="currentColor" />{votes.cheer.length}</span>
							{#if hub.party.rules.skip === 'vote'}<span class="label text-[13px] text-muted">Skip {votes.skip.length}/{votes.need}</span>{/if}
						</div>
					{/if}
				</div>
			</div>
		</section>

		<aside class="flex shrink-0 flex-col border-line max-md:border-t md:w-[19rem] md:border-l">
			<h2 class="label flex h-10 shrink-0 items-center border-b border-line px-4 text-[12px] text-muted">Up next</h2>
			<ol class="min-h-0 flex-1 overflow-hidden">
				{#each next as block (block.key)}
					{@const first = block.items[0]}
					{@const what = radio.show(first)}
					<li class="flex items-center gap-3 border-b border-line pr-4">
						<Cover art={what.art} src={what.src} class="size-16 shrink-0" />
						<span class="min-w-0 flex-1">
							<span class="wide block truncate text-[15px] leading-5">{block.album ? first.album : what.title}</span>
							<span class="block truncate text-[13px] leading-4 text-muted">{block.album ? `${first.artist} · ${block.items.length} songs` : what.subtitle}</span>
						</span>
						<Sign person={line.person(block.pick)} size="sm" />
					</li>
				{:else}
					<li class="px-4 py-3 text-[15px] text-muted">Nothing yet. Be the first caller.</li>
				{/each}
			</ol>
			{#if line.open}
				<div class="flex shrink-0 items-center gap-4 border-t border-line p-4">
					<QrCode text={url} class="size-32 shrink-0" />
					<div class="min-w-0">
						<p class="wide text-[19px] leading-tight">Scan to request</p>
						<p class="mt-1 text-[14px] break-all text-muted">{url.replace(/^https?:\/\//, '')}</p>
					</div>
				</div>
			{/if}
		</aside>
	</div>
</div>
