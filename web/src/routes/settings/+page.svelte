<script lang="ts">
	import { Check, House, LoaderCircle, RefreshCw, RotateCw, TriangleAlert, Usb } from '@lucide/svelte';
	import LinePanel from '#lib/components/LinePanel.svelte';
	import Masthead from '#lib/components/Masthead.svelte';
	import YouPanel from '#lib/components/YouPanel.svelte';
	import SignalPathCard from '#lib/components/SignalPathCard.svelte';
	import { api } from '#lib/api.ts';
	import { Confirm } from '#lib/confirm.svelte.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { line } from '#lib/party.svelte.ts';
	import { reach, type Hand, type ReachMode } from '#lib/reach.svelte.ts';
	import { theme, type ThemeMode } from '#lib/theme.svelte.ts';
	import { toasts } from '#lib/toast.svelte.ts';

	const reaches: { id: ReachMode; label: string; about: string }[] = [
		{ id: 'off', label: 'Off', about: 'Tabs at the bottom, controls at the top of each page.' },
		{ id: 'deck', label: 'Deck', about: 'All six tabs at the bottom, and each page’s controls in a row right above the player.' },
		{ id: 'rail', label: 'Rail', about: 'A column of keys down the edge of the screen, in place of the tabs.' },
		{ id: 'arc', label: 'Arc', about: 'A dial in the corner: press it and slide onto a key, in place of the tabs.' }
	];
	const hands: Hand[] = ['right', 'left'];
	const modes: { id: ThemeMode; label: string }[] = [
		{ id: 'system', label: 'System' },
		{ id: 'light', label: 'Light' },
		{ id: 'dark', label: 'Dark' }
	];

	let rescanning = $state(false);

	async function rescan() {
		rescanning = true;
		try {
			await api.rescan();
		} finally {
			setTimeout(() => (rescanning = false), 1200);
		}
	}

	let outputs = $derived(hub.outputs.outputs);
	let newCards = $derived(hub.outputs.newCards);
	let switching = $state<number | null>(null);
	let restarting = $state(false);

	async function selectOutput(id: number) {
		switching = id;
		try {
			await api.selectOutput(id);
		} finally {
			switching = null;
		}
	}

	async function restart() {
		restarting = true;
		try {
			await api.restart();
		} catch {
			restarting = false;
		}
	}

	// The restart is over once the connection has dropped and come back.
	let dropped = false;
	$effect(() => {
		if (!restarting) return;
		if (!hub.connected) dropped = true;
		else if (dropped) {
			dropped = false;
			restarting = false;
		}
	});

	let airplay = $derived(hub.source.available.includes('airplay'));

	let homekit = $derived(hub.homekit);
	const resetting = new Confirm();
	const resetHomekit = () => api.resetHomekit().then(() => toasts.done('Virgil can be added to a home again'));
</script>

<svelte:head><title>Settings · Virgil</title></svelte:head>

{#snippet section(title: string)}
	<h2 class="label flex h-9 items-center border-y border-line bg-surface px-3 text-[11px] text-muted">{title}</h2>
{/snippet}

<div class="pt-safe lg:max-w-3xl">
	<Masthead word="SETTINGS" />

	{@render section('Appearance')}
	<div class="grid h-14 grid-cols-3">
		{#each modes as mode (mode.id)}
			<button
				type="button"
				class="wide border-l border-line text-xs uppercase first:border-l-0 {theme.mode === mode.id ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
				aria-pressed={theme.mode === mode.id}
				onclick={() => theme.set(mode.id)}
			>
				{mode.label}
			</button>
		{/each}
	</div>

	<!-- Phones only: a larger screen is held in two hands, or sits on a table. -->
	<div class="md:hidden">
		{@render section('One-handed layout')}
		<div class="grid h-14 grid-cols-4" role="radiogroup" aria-label="One-handed layout">
			{#each reaches as mode (mode.id)}
				<button
					type="button"
					role="radio"
					class="wide border-l border-line text-xs uppercase first:border-l-0 {reach.mode === mode.id ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
					aria-checked={reach.mode === mode.id}
					onclick={() => reach.set(mode.id)}
				>
					{mode.label}
				</button>
			{/each}
		</div>
		<p class="border-t border-line px-3 py-2.5 text-[13px] text-muted">{reaches.find((m) => m.id === reach.mode)?.about}</p>
		{#if reach.mode === 'rail' || reach.mode === 'arc'}
			<div class="grid h-11 grid-cols-2 border-t border-line" role="radiogroup" aria-label="Hand">
				{#each hands as hand (hand)}
					<button
						type="button"
						role="radio"
						class="label border-l border-line text-[11px] first:border-l-0 {reach.hand === hand ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
						aria-checked={reach.hand === hand}
						onclick={() => reach.setHand(hand)}
					>
						{hand} hand
					</button>
				{/each}
			</div>
		{/if}
	</div>

	{@render section('Request line')}
	<LinePanel />

	{@render section('You')}
	<YouPanel />

	<!-- Guests on the request line: the rest is the host's. -->
	{#if line.host}
	{@render section('Output')}
	{#if outputs.length > 1 || newCards.length}
		<div class="divide-y divide-line border-b border-line" role="radiogroup" aria-label="Output device">
			{#each outputs as output (output.id)}
				<button
					type="button"
					role="radio"
					aria-checked={output.enabled}
					class="press flex min-h-14 w-full items-center justify-between gap-4 px-3 text-left"
					disabled={switching !== null}
					onclick={() => !output.enabled && selectOutput(output.id)}
				>
					<span class="min-w-0">
						<span class="wide block truncate text-[13px] leading-[18px]">{output.label}</span>
						{#if output.label !== output.name}
							<span class="block truncate text-xs leading-4 text-muted">{output.name}</span>
						{/if}
					</span>
					{#if switching === output.id}
						<LoaderCircle class="size-4 shrink-0 text-muted" />
					{:else if output.enabled}
						<Check class="size-5 shrink-0 text-accent" />
					{/if}
				</button>
			{/each}
			{#if newCards.length}
				<div class="flex items-stretch">
					<div class="flex min-w-0 flex-1 items-center gap-3 px-3 py-2.5">
						<Usb class="size-4 shrink-0 text-info" />
						<div class="min-w-0">
							<div class="truncate text-sm font-medium">{newCards.join(', ')}</div>
							<div class="text-xs text-muted">Connected since Virgil started. Restart Virgil to use it.</div>
						</div>
					</div>
					<button type="button" class="press label flex shrink-0 items-center gap-2 border-l border-line px-3 text-[10.5px] disabled:text-faint" disabled={restarting} onclick={restart}>
						<RotateCw class="size-3.5" />
						{restarting ? 'Restarting…' : 'Restart'}
					</button>
				</div>
			{/if}
		</div>
	{/if}
	<SignalPathCard signal={hub.signal} />
	<div class="flex min-h-11 items-center justify-between gap-4 border-t border-line px-3">
		<span class="label text-[11px] text-muted">Playback engine</span>
		<span class="label flex items-center gap-1.5 text-[10px] {hub.player.online ? 'text-good' : 'text-warn'}">
			<span class="size-1.5 rounded-full bg-current"></span>
			{hub.player.online ? 'MPD connected' : 'MPD unreachable'}
		</span>
	</div>

	{@render section('Library')}
	<div class="flex items-stretch border-b border-line">
		<div class="min-w-0 flex-1 px-3 py-2.5">
			<div class="text-sm font-medium">
				{hub.library.albums.toLocaleString()} albums · {hub.library.tracks.toLocaleString()} songs
			</div>
			<div class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
				{#if hub.library.scanning}
					<LoaderCircle class="size-3" /> Scanning and fetching artwork…
				{:else}
					Up to date with MPD's database
				{/if}
			</div>
		</div>
		<button
			type="button"
			class="press label flex shrink-0 items-center gap-2 border-l border-line px-3 text-[10.5px] disabled:text-faint"
			disabled={rescanning || !hub.player.online}
			onclick={rescan}
		>
			<RefreshCw class="size-3.5" /> Rescan
		</button>
	</div>
	<div class="flex items-stretch border-b border-line">
		<div class="min-w-0 flex-1 px-3 py-2.5">
			<div class="text-sm font-medium">Album artwork</div>
			<div class="mt-0.5 text-xs text-muted">
				{#if hub.library.missingArt}
					{hub.library.missingArt.toLocaleString()}
					{hub.library.missingArt === 1 ? 'album has' : 'albums have'} no artwork
				{:else}
					Every album has artwork
				{/if}
			</div>
		</div>
	</div>

	{#if airplay}
		{@render section('AirPlay')}
		<div class="border-b border-line px-3 py-2.5">
			<div class="text-sm font-medium">{hub.external ? 'Streaming now' : 'Ready'}</div>
			<p class="mt-0.5 text-[13px] text-muted">
				This player is an AirPlay 2 speaker. Group it with other rooms from Control Center on an iPhone or Mac.
				Starting playback here takes the DAC back from AirPlay.
			</p>
		</div>
	{/if}

	{#if homekit}
		{@render section('Apple Home')}
		<div class="border-b border-line px-3 py-2.5">
			{#if homekit.paired}
				<div class="text-sm font-medium">In your home</div>
				<p class="mt-0.5 text-[13px] text-muted">
					Say “Hey Siri, turn off Virgil” to stop playback, and “turn on Virgil” to resume. Rename it or give it
					a room in the Home app.
				</p>
			{:else}
				<div class="text-sm font-medium">Not in a home yet</div>
				<p class="mt-0.5 text-[13px] text-muted">
					Add Virgil to the Home app, then say “Hey Siri, turn off Virgil” to stop playback. The Home app calls it
					uncertified; add it anyway.
				</p>
				<div class="mt-3 font-mono text-2xl select-all">{homekit.setupCode}</div>
				<a href={homekit.setupUri} class="press wide mt-3 flex h-12 items-center justify-center gap-2 bg-accent text-[13px] text-accent-fg uppercase">
					<House class="size-4" /> Add to Home
				</a>
				<p class="mt-2 text-xs text-muted">Or in the Home app: Add Accessory › More options…, then enter the code.</p>
			{/if}
			{#if !homekit.advertised}
				<p class="mt-2 flex items-start gap-1.5 text-xs text-warn">
					<TriangleAlert class="mt-px size-3.5 shrink-0" />
					Not visible on the network yet. Is avahi-daemon running on the Pi?
				</p>
			{/if}
		</div>
		{#if homekit.paired}
			<div class="flex items-stretch border-b border-line">
				<div class="min-w-0 flex-1 px-3 py-2.5 text-xs text-muted">
					Removed it in the Home app while Virgil was off? Reset so it can be added again.
				</div>
				<button
					type="button"
					class="label shrink-0 border-l border-line px-3 text-[10.5px] whitespace-nowrap {resetting.armed ? 'press bg-accent text-accent-fg' : 'press'}"
					onclick={() => resetting.press(resetHomekit)}
				>
					{resetting.armed ? 'Confirm' : 'Reset'}
				</button>
			</div>
		{/if}
	{/if}
	{/if}
</div>
