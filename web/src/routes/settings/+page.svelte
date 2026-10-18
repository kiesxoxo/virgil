<script lang="ts">
	import { Airplay, Check, ImageDown, LoaderCircle, Monitor, Moon, Power, RefreshCw, RotateCw, Sun, Usb } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import SignalPathCard from '#lib/components/SignalPathCard.svelte';
	import { api } from '#lib/api.ts';
	import { hub } from '#lib/hub.svelte.ts';
	import { theme, type ThemeMode } from '#lib/theme.svelte.ts';
	import type { DesktopSettings } from '#lib/types/DesktopSettings.ts';

	// Desktop mode (Virgil.exe on a PC): Virgil's own settings, and a way to quit.
	let desktop = $state<DesktopSettings | null>(null);
	let musicDir = $state('');
	let applying = $state(false);
	let quitting = $state(false);

	onMount(() => {
		api.desktop().then(
			(settings) => {
				desktop = settings;
				musicDir = settings.musicDir;
			},
			() => {} // not desktop mode
		);
	});

	async function applyDesktop() {
		if (!desktop) return;
		applying = true;
		try {
			await api.saveDesktop({ ...desktop, musicDir: musicDir.trim() });
			desktop = { ...desktop, musicDir: musicDir.trim() };
			restarting = true;
		} finally {
			applying = false;
		}
	}

	async function quit() {
		quitting = true;
		await api.quit().catch(() => (quitting = false));
	}

	const modes: { id: ThemeMode; label: string; icon: typeof Sun }[] = [
		{ id: 'system', label: 'System', icon: Monitor },
		{ id: 'light', label: 'Light', icon: Sun },
		{ id: 'dark', label: 'Dark', icon: Moon }
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
	let download = $derived(hub.library.artDownload);
	let downloading = $derived(!!download && !download.error && download.done < download.total);
</script>

<svelte:head><title>Settings · Virgil</title></svelte:head>

{#snippet section(title: string)}
	<h2 class="mt-8 mb-2 px-1 text-xs font-semibold tracking-wider text-muted uppercase">{title}</h2>
{/snippet}

<div class="pt-safe mx-auto max-w-2xl px-4 pt-4 lg:px-8 lg:pt-8">
	<h1 class="text-[28px] font-bold tracking-tight lg:text-[32px]">Settings</h1>

	{@render section('Appearance')}
	<div class="grid grid-cols-3 gap-1 rounded-2xl bg-surface p-1">
		{#each modes as mode (mode.id)}
			<button
				type="button"
				class="press flex h-11 items-center justify-center gap-2 rounded-xl text-sm font-semibold transition-colors {theme.mode === mode.id
					? 'bg-bg text-fg shadow-sm ring-1 ring-line'
					: 'text-muted hover:text-fg'}"
				aria-pressed={theme.mode === mode.id}
				onclick={() => theme.set(mode.id)}
			>
				<mode.icon class="size-4" />
				{mode.label}
			</button>
		{/each}
	</div>

	{@render section('Output')}
	{#if outputs.length > 1 || newCards.length}
		<div class="mb-2 divide-y divide-line rounded-2xl bg-surface" role="radiogroup" aria-label="Output device">
			{#each outputs as output (output.id)}
				<button
					type="button"
					role="radio"
					aria-checked={output.enabled}
					class="flex w-full items-center justify-between gap-4 px-4 py-3.5 text-left"
					disabled={switching !== null}
					onclick={() => !output.enabled && selectOutput(output.id)}
				>
					<span class="min-w-0">
						<span class="block truncate text-sm font-medium">{output.label}</span>
						{#if output.label !== output.name}
							<span class="block truncate text-xs text-muted">{output.name}</span>
						{/if}
					</span>
					{#if switching === output.id}
						<LoaderCircle class="size-4 shrink-0 animate-spin text-muted" />
					{:else if output.enabled}
						<Check class="size-5 shrink-0 text-accent" />
					{/if}
				</button>
			{/each}
			{#if newCards.length}
				<div class="flex items-center justify-between gap-4 px-4 py-3.5">
					<div class="flex min-w-0 items-center gap-3">
						<Usb class="size-4 shrink-0 text-info" />
						<div class="min-w-0">
							<div class="truncate text-sm font-medium">{newCards.join(', ')}</div>
							<div class="text-xs text-muted">Connected since Virgil started. Restart Virgil to use it.</div>
						</div>
					</div>
					<button
						type="button"
						class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold ring-1 ring-line disabled:opacity-50"
						disabled={restarting}
						onclick={restart}
					>
						<RotateCw class="size-3.5 {restarting ? 'animate-spin' : ''}" />
						{restarting ? 'Restarting…' : 'Restart'}
					</button>
				</div>
			{/if}
		</div>
	{/if}
	<SignalPathCard signal={hub.signal} />
	<div class="mt-2 divide-y divide-line rounded-2xl bg-surface">
		<div class="flex items-center justify-between gap-4 px-4 py-3.5">
			<span class="text-sm font-medium">Volume</span>
			<span class="text-right text-sm text-muted">
				{#if hub.player.volume == null}
					Fixed — set it on your DAC or amp
				{:else}
					{hub.player.volume}% <span class="text-warn">(MPD mixer active)</span>
				{/if}
			</span>
		</div>
		<div class="flex items-center justify-between gap-4 px-4 py-3.5">
			<span class="text-sm font-medium">Playback engine</span>
			<span class="flex items-center gap-2 text-sm text-muted">
				<span class="size-2 rounded-full {hub.player.online ? 'bg-good' : 'bg-warn'}"></span>
				{hub.player.online ? 'MPD connected' : 'MPD unreachable'}
			</span>
		</div>
	</div>

	{@render section('Library')}
	<div class="divide-y divide-line rounded-2xl bg-surface">
	<div class="flex items-center justify-between gap-4 px-4 py-3.5">
		<div>
			<div class="text-sm font-medium">
				{hub.library.albums.toLocaleString()} albums · {hub.library.tracks.toLocaleString()} songs
			</div>
			<div class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
				{#if hub.library.scanning}
					<LoaderCircle class="size-3 animate-spin" /> Scanning and fetching artwork…
				{:else}
					Up to date with MPD's database
				{/if}
			</div>
		</div>
		<button
			type="button"
			class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold ring-1 ring-line disabled:opacity-50"
			disabled={rescanning || !hub.player.online}
			onclick={rescan}
		>
			<RefreshCw class="size-3.5 {rescanning ? 'animate-spin' : ''}" /> Rescan
		</button>
	</div>
	<div class="flex items-center justify-between gap-4 px-4 py-3.5">
		<div class="min-w-0">
			<div class="text-sm font-medium">Album artwork</div>
			<div class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
				{#if downloading && download}
					<LoaderCircle class="size-3 shrink-0 animate-spin" />
					Downloading… {download.done} of {download.total}, {download.saved} saved
				{:else if download?.error}
					<span class="text-warn">Stopped: {download.error}</span>
				{:else if download?.failed}
					<span class="text-warn">
						Saved {download.saved}; {download.failed} couldn't be saved (is the album folder writable?)
					</span>
				{:else if hub.library.missingArt}
					{hub.library.missingArt.toLocaleString()}
					{hub.library.missingArt === 1 ? 'album has' : 'albums have'} no artwork
				{:else}
					Every album has artwork
				{/if}
			</div>
		</div>
		<button
			type="button"
			class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold ring-1 ring-line disabled:opacity-50"
			disabled={downloading || !hub.library.missingArt}
			title="Saves cover.jpg in each album folder that has no embedded or folder art"
			onclick={() => api.downloadMissingArt()}
		>
			<ImageDown class="size-3.5" /> Download missing
		</button>
	</div>
	</div>

	{#if desktop}
		{@render section('This computer')}
		<div class="divide-y divide-line rounded-2xl bg-surface">
			<form
				class="px-4 py-3.5"
				onsubmit={(e) => {
					e.preventDefault();
					applyDesktop();
				}}
			>
				<label for="music-dir" class="text-sm font-medium">Music folder</label>
				<div class="mt-2 flex gap-2">
					<input
						id="music-dir"
						class="h-9 min-w-0 flex-1 rounded-xl bg-bg px-3 text-base ring-1 ring-line outline-none focus:ring-2 focus:ring-accent sm:text-sm"
						bind:value={musicDir}
						spellcheck="false"
						autocomplete="off"
						placeholder="C:\Users\you\Music or \\server\share\Music"
					/>
					<button
						type="submit"
						class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold ring-1 ring-line disabled:opacity-50"
						disabled={applying || restarting || !musicDir.trim() || musicDir.trim() === desktop.musicDir}
					>
						{#if applying || restarting}<LoaderCircle class="size-3.5 animate-spin" />{/if}
						Apply
					</button>
				</div>
				<p class="mt-1.5 text-xs text-muted">Virgil restarts and scans the new folder.</p>
			</form>
			<div class="flex items-center justify-between gap-4 px-4 py-3.5">
				<div>
					<div class="text-sm font-medium">Quit Virgil</div>
					<div class="mt-0.5 text-xs text-muted">Stops playback and closes Virgil until you open it again.</div>
				</div>
				<button
					type="button"
					class="press inline-flex h-9 shrink-0 items-center gap-2 rounded-full bg-bg px-4 text-[13px] font-semibold text-accent ring-1 ring-line disabled:opacity-50"
					disabled={quitting}
					onclick={quit}
				>
					<Power class="size-3.5" />
					{quitting ? 'Quitting…' : 'Quit'}
				</button>
			</div>
		</div>
	{/if}

	{#if airplay}
		{@render section('AirPlay')}
		<div class="flex gap-4 rounded-2xl bg-surface px-4 py-3.5">
			<div class="grid size-10 shrink-0 place-items-center rounded-xl bg-info-bg text-info"><Airplay class="size-5" /></div>
			<div>
				<div class="text-sm font-medium">{hub.external ? 'Streaming now' : 'Ready'}</div>
				<p class="mt-0.5 text-[13px] text-muted">
					This player is an AirPlay 2 speaker. Group it with other rooms from Control Center on an iPhone or Mac.
					Starting playback here takes the DAC back from AirPlay.
				</p>
			</div>
		</div>
	{/if}
</div>
