<script lang="ts" module>
	import type { Component } from 'svelte';

	export interface MenuItem {
		label: string;
		icon: Component<{ class?: string }>;
		run: () => unknown;
	}
</script>

<script lang="ts">
	import { Ellipsis } from '@lucide/svelte';

	let { items, label = 'More actions', class: className = '' }: { items: MenuItem[]; label?: string; class?: string } =
		$props();

	let open = $state(false);
	let button: HTMLButtonElement;
	let pos = $state({ top: 0, left: 0, up: false });

	function toggle(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		if (open) {
			open = false;
			return;
		}
		const r = button.getBoundingClientRect();
		const up = r.bottom + 56 * items.length + 120 > window.innerHeight;
		pos = { top: up ? r.top : r.bottom, left: Math.max(8, Math.min(window.innerWidth - 216, r.right - 208)), up };
		open = true;
	}
</script>

<svelte:window
	onclick={() => (open = false)}
	onscroll={() => (open = false)}
	onresize={() => (open = false)}
	onkeydown={(e) => e.key === 'Escape' && (open = false)}
/>

<button
	bind:this={button}
	type="button"
	class="press grid size-9 shrink-0 place-items-center rounded-full text-muted hover:bg-fg/6 hover:text-fg {className}"
	aria-label={label}
	aria-haspopup="menu"
	aria-expanded={open}
	onclick={toggle}
>
	<Ellipsis class="size-[18px]" />
</button>

{#if open}
	<div
		role="menu"
		class="glass fixed z-50 w-52 rounded-2xl p-1.5 shadow-float ring-1 ring-line"
		style:top="{pos.top}px"
		style:left="{pos.left}px"
		style:transform={pos.up ? 'translateY(calc(-100% - 6px))' : 'translateY(6px)'}
	>
		{#each items as item (item.label)}
			<button
				type="button"
				role="menuitem"
				class="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm font-medium hover:bg-fg/6"
				onclick={(e) => {
					e.stopPropagation();
					open = false;
					item.run();
				}}
			>
				<item.icon class="size-4 text-muted" />
				{item.label}
			</button>
		{/each}
	</div>
{/if}
