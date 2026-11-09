<script lang="ts">
	import { ChevronLeft, Search, SlidersHorizontal } from '@lucide/svelte';
	import { flushSync } from 'svelte';
	import { api } from '../api.ts';
	import { hub } from '../hub.svelte.ts';
	import { keyboard } from '../keyboard.svelte.ts';
	import { reach } from '../reach.svelte.ts';
	import { search } from '../search.svelte.ts';
	import { followBackLink } from '../swipeback.ts';
	import Glyph from './Glyph.svelte';

	// The rail: a column of keys down the thumb's edge of the screen, with the ones used most in the
	// middle of its sweep and the rare ones furthest away. They're the same on every page, so a thumb
	// learns them, except for one key that sorts in the library and goes back from a detail page.
	let { items, routeId }: { items: { href: string; label: string; active: (id: string) => boolean }[]; routeId: string } = $props();

	let keys = $derived(items.toReversed());
	let library = $derived(routeId === '/' || routeId === '/artists' || routeId === '/genres');
	let detail = $derived(routeId.startsWith('/album/') || routeId.startsWith('/artist/') || routeId.startsWith('/genre/'));
	const cell = 'flex min-h-10 max-h-[52px] flex-1 flex-col items-center justify-center gap-1 border-t border-line';
</script>

<div
	class="pt-safe pb-safe fixed inset-y-0 z-30 flex w-14 flex-col justify-end border-line bg-bg {reach.hand === 'right' ? 'right-0 border-l' : 'left-0 border-r'}"
	class:hidden={keyboard.height > 0}
>
	<span class="label mb-auto self-center pt-4 text-[9px] tracking-[0.3em] text-faint [writing-mode:vertical-rl]">Virgil</span>

	{#each keys as item (item.href)}
		{@const active = item.active(routeId)}
		<a href={item.href} class="{cell} label text-[9px] {active ? 'bg-fg text-bg' : 'press text-muted'}" aria-current={active ? 'page' : undefined}>
			{item.label}
		</a>
	{/each}

	<button
		type="button"
		class="{cell} press label border-t-[3px] text-[9px] text-muted"
		onclick={() => {
			search.open = true;
			// iOS only raises the keyboard for a focus() made during the tap itself.
			flushSync();
		}}
	>
		<Search class="size-[18px]" />
	</button>

	{#if detail}
		<button type="button" class="{cell} press label text-[9px]" onclick={followBackLink}>
			<ChevronLeft class="size-[18px]" /> Back
		</button>
	{:else if library}
		<button
			type="button"
			data-panel-key
			class="{cell} label text-[9px] {reach.panel ? 'press-lit bg-fg text-bg' : 'press text-muted'}"
			aria-pressed={reach.panel}
			onclick={() => (reach.panel = !reach.panel)}
		>
			<SlidersHorizontal class="size-[18px]" /> Sort
		</button>
	{:else}
		<span class={cell}></span>
	{/if}

	<button type="button" class="{cell} press disabled:text-faint" aria-label="Next" disabled={!hub.current} onclick={() => api.next()}>
		<Glyph name="next" class="size-5" />
	</button>
	<button
		type="button"
		class="press flex min-h-12 max-h-[72px] flex-[1.4] items-center justify-center border-t border-line bg-accent text-accent-fg disabled:bg-surface-2 disabled:text-faint"
		aria-label={hub.playing ? 'Pause' : 'Play'}
		disabled={!hub.current}
		onclick={() => hub.toggle()}
	>
		<Glyph name={hub.playing ? 'pause' : 'play'} class="size-6" />
	</button>
</div>
