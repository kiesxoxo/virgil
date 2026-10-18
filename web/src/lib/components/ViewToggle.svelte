<script lang="ts">
	import { Image, ImageOff, LayoutGrid, List } from '@lucide/svelte';

	// Grid or list, plus artwork on or off while it's a list. Shared by the
	// Albums, Artists and Genres pages.
	let { view = $bindable(), art = $bindable() }: { view: 'grid' | 'list'; art: boolean } = $props();
</script>

{#if view === 'list'}
	<button
		type="button"
		class="press grid size-8 place-items-center rounded-lg bg-surface {art ? 'text-fg' : 'text-muted'} hover:text-fg"
		aria-pressed={art}
		aria-label={art ? 'Hide artwork' : 'Show artwork'}
		title={art ? 'Hide artwork' : 'Show artwork'}
		onclick={() => (art = !art)}
	>
		{#if art}<Image class="size-4" />{:else}<ImageOff class="size-4" />{/if}
	</button>
{/if}
<div class="flex rounded-lg bg-surface p-0.5" role="group" aria-label="Layout">
	{#each [{ id: 'grid', label: 'Grid', icon: LayoutGrid }, { id: 'list', label: 'List', icon: List }] as const as option (option.id)}
		<button
			type="button"
			class="grid size-7 place-items-center rounded-md transition-colors {view === option.id
				? 'bg-bg text-fg shadow-sm ring-1 ring-line'
				: 'text-muted hover:text-fg'}"
			aria-pressed={view === option.id}
			aria-label="{option.label} view"
			title="{option.label} view"
			onclick={() => (view = option.id)}
		>
			<option.icon class="size-4" />
		</button>
	{/each}
</div>
