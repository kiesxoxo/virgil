<script lang="ts" generics="V extends AlbumView">
	import { Image, ImageOff, LayoutGrid, List, Radio } from '@lucide/svelte';
	import type { AlbumView } from '../stored.ts';

	// Grid or list (the albums also have the tuner), plus artwork on or off while it's a list. Shared
	// by the Albums, Artists and Genres pages.
	let { view = $bindable(), art = $bindable(), tuner = false }: { view: V; art: boolean; tuner?: boolean } = $props();

	const options = [
		{ id: 'grid', label: 'Grid', icon: LayoutGrid },
		{ id: 'list', label: 'List', icon: List },
		{ id: 'tuner', label: 'Tuner', icon: Radio }
	] as const;
	let shown = $derived(tuner ? options : options.slice(0, 2));
</script>

{#if view === 'list'}
	<button
		type="button"
		class="press grid w-11 shrink-0 place-items-center border-l border-line {art ? 'text-fg' : 'text-muted'}"
		aria-pressed={art}
		aria-label={art ? 'Hide artwork' : 'Show artwork'}
		title={art ? 'Hide artwork' : 'Show artwork'}
		onclick={() => (art = !art)}
	>
		{#if art}<Image class="size-[18px]" />{:else}<ImageOff class="size-[18px]" />{/if}
	</button>
{/if}
{#each shown as option (option.id)}
	<button
		type="button"
		class="press grid w-11 shrink-0 place-items-center border-l border-line {view === option.id ? 'text-accent' : 'text-muted hover:text-fg'}"
		aria-pressed={view === option.id}
		aria-label="{option.label} view"
		title="{option.label} view"
		onclick={() => (view = option.id as V)}
	>
		<option.icon class="size-[18px]" />
	</button>
{/each}
