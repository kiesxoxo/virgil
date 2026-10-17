<script lang="ts">
	import { Ellipsis } from '@lucide/svelte';
	import { menu, type MenuHeader, type MenuItem } from '../menu.svelte.ts';

	let {
		items,
		header = null,
		label = 'More actions',
		class: className = ''
	}: { items: () => MenuItem[]; header?: MenuHeader | null; label?: string; class?: string } = $props();

	let button: HTMLButtonElement;
</script>

<button
	bind:this={button}
	type="button"
	class="press grid size-9 shrink-0 place-items-center rounded-full text-muted hover:bg-fg/6 hover:text-fg {className}"
	aria-label={label}
	aria-haspopup="menu"
	onclick={(e) => {
		e.preventDefault();
		e.stopPropagation();
		menu.show(items(), button.getBoundingClientRect(), header);
	}}
>
	<Ellipsis class="size-[18px]" />
</button>
