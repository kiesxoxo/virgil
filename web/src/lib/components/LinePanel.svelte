<script lang="ts">
	// The request line: open or close it, invite people (a QR code), its rules, who's who.
	import { ChevronRight, Monitor, Phone, PhoneOff, Radio } from '@lucide/svelte';
	import { api } from '../api.ts';
	import { Confirm } from '../confirm.svelte.ts';
	import { hub } from '../hub.svelte.ts';
	import { line } from '../party.svelte.ts';
	import type { Rules } from '../types/Rules.ts';
	import QrCode from './QrCode.svelte';
	import Sign from './Sign.svelte';

	let { onnavigate }: { onnavigate?: () => void } = $props();

	let rules = $derived(hub.party.rules);
	let busy = $state(false);
	const url = typeof location === 'undefined' ? '' : location.origin;
	const forgetting = new Map<string, Confirm>();
	const confirmFor = (id: string) => forgetting.get(id) ?? forgetting.set(id, new Confirm()).get(id)!;

	async function toggle() {
		busy = true;
		try {
			await line.setOpen(!line.open);
		} finally {
			busy = false;
		}
	}

	const setRules = (change: Partial<Rules>) => api.party({ rules: { ...rules, ...change } });

	// Hosts first, then whoever is here, then by name.
	let people = $derived(
		[...hub.party.people].sort(
			(a, b) =>
				Number(b.host) - Number(a.host) ||
				Number(hub.party.here.includes(b.id)) - Number(hub.party.here.includes(a.id)) ||
				a.name.localeCompare(b.name)
		)
	);

	const ruleRows: { label: string; keys: { label: string; on: boolean; set: Partial<Rules> }[] }[] = $derived([
		{
			label: 'Order',
			keys: [
				{ label: 'Take turns', on: rules.takeTurns, set: { takeTurns: true } },
				{ label: 'As they come', on: !rules.takeTurns, set: { takeTurns: false } }
			]
		},
		{
			label: 'Skipping',
			keys: [
				{ label: 'By vote', on: rules.skip === 'vote', set: { skip: 'vote' } },
				{ label: 'Anyone', on: rules.skip === 'free', set: { skip: 'free' } },
				{ label: 'Hosts', on: rules.skip === 'off', set: { skip: 'off' } }
			]
		},
		{
			label: 'Guests pause',
			keys: [
				{ label: 'Yes', on: rules.guestsPause, set: { guestsPause: true } },
				{ label: 'No', on: !rules.guestsPause, set: { guestsPause: false } }
			]
		}
	]);
</script>

{#if line.host}
	<button
		type="button"
		class="wide flex h-14 w-full items-center justify-center gap-2.5 text-[15px] uppercase disabled:text-faint {line.open ? 'press border-b border-line' : 'press bg-accent text-accent-fg'}"
		disabled={busy}
		onclick={toggle}
	>
		{#if line.open}<PhoneOff class="size-4" strokeWidth={2.5} /> Close the line{:else}<Phone class="size-4" strokeWidth={2.5} /> Open the line{/if}
	</button>
{/if}
{#if !line.open}
	<p class="border-b border-line px-3 py-2.5 text-[13px] text-muted">
		Open the line and friends call in from their phones: they request songs and albums, which take turns with everyone else's, and vote on what plays. Your own devices stay hosts.
	</p>
{/if}

{#if line.open}
	<div class="flex items-center gap-3 border-b border-line p-3">
		<QrCode text={url} class="size-32 shrink-0 border border-line" />
		<div class="min-w-0">
			<p class="label flex items-center gap-1.5 text-[11px] text-accent"><span class="size-1.5 rounded-full bg-current"></span>Line open</p>
			<p class="wide mt-1 text-[17px] leading-tight">Scan to request</p>
			<p class="mt-1 text-[13px] break-all text-muted">{url.replace(/^https?:\/\//, '')}</p>
		</div>
	</div>
	<div class="grid grid-cols-2 border-b border-line">
		<a href="/screen" class="press label flex h-11 items-center justify-center gap-2 text-[11px]" onclick={onnavigate}><Monitor class="size-4" /> Big screen</a>
		<a href="/aircheck" class="press label flex h-11 items-center justify-center gap-2 border-l border-line text-[11px]" onclick={onnavigate}><Radio class="size-4" /> Aircheck</a>
	</div>
{:else}
	<a href="/aircheck" class="press flex h-11 items-center justify-between border-b border-line px-3" onclick={onnavigate}>
		<span class="text-sm font-medium">The last night’s aircheck</span><ChevronRight class="size-4 text-muted" />
	</a>
{/if}

{#if line.host}
	{#each ruleRows as row (row.label)}
		<div class="flex h-11 items-stretch border-b border-line">
			<span class="label flex w-28 shrink-0 items-center px-3 text-[10px] text-muted">{row.label}</span>
			{#each row.keys as key (key.label)}
				<button
					type="button"
					class="label flex-1 border-l border-line text-[10.5px] {key.on ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
					aria-pressed={key.on}
					onclick={() => setRules(key.set)}
				>
					{key.label}
				</button>
			{/each}
		</div>
	{/each}
{/if}

{#if people.length}
	<h3 class="label flex h-9 items-center justify-between border-b border-line px-3 text-[11px] text-muted">
		<span>People</span><span>{hub.party.here.length} here</span>
	</h3>
	<ul class="divide-y divide-line border-b border-line">
		{#each people as person (person.id)}
			{@const here = hub.party.here.includes(person.id)}
			{@const forget = confirmFor(person.id)}
			<li class="flex min-h-12 items-stretch">
				<div class="flex min-w-0 flex-1 items-center gap-3 px-3 py-2">
					<Sign {person} size="sm" />
					<span class="min-w-0">
						<span class="block truncate text-sm font-medium">{person.name}{person.id === line.me?.id ? ' (you)' : ''}</span>
						<span class="label block text-[10px] {here ? 'text-good' : 'text-faint'}">{here ? 'Here' : 'Away'}{person.host ? ' · Host' : ''}</span>
					</span>
				</div>
				{#if line.self?.host && person.id !== line.me?.id}
					<button
						type="button"
						class="label w-16 border-l border-line text-[10px] {person.host ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
						aria-pressed={person.host}
						onclick={() => api.setHost(person.id, !person.host)}
					>
						Host
					</button>
					<button
						type="button"
						class="label w-20 border-l border-line text-[10px] {forget.armed ? 'press bg-accent text-accent-fg' : 'press text-muted'}"
						onclick={() => forget.press(() => api.forget(person.id))}
					>
						{forget.armed ? 'Confirm' : 'Forget'}
					</button>
				{/if}
			</li>
		{/each}
	</ul>
{/if}
