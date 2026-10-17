<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { menu } from '../menu.svelte.ts';
	import Cover from './Cover.svelte';

	const WIDTH = 232;
	const ITEM = 40;

	let width = $state(0);
	let height = $state(0);
	// Phones get a bottom sheet with big targets; larger screens a dropdown.
	let sheet = $derived(width < 640);

	let left = $derived(Math.max(8, Math.min(menu.alignRight ? menu.x - WIDTH : menu.x, width - WIDTH - 8)));
	let estimated = $derived(menu.items.length * ITEM + (menu.header ? 48 : 0) + 12);
	let top = $derived(menu.y + estimated > height - 8 ? Math.max(8, menu.yAbove - estimated) : menu.y);

	/** The finger that long-pressed lifts onto the backdrop; don't treat that as "dismiss". */
	function dismiss() {
		if (performance.now() - menu.openedAt > 350) menu.close();
	}
</script>

<svelte:window
	bind:innerWidth={width}
	bind:innerHeight={height}
	onkeydown={(e) => menu.open && e.key === 'Escape' && menu.close()}
	onresize={() => menu.close()}
/>

{#if menu.open}
	{#if sheet}
		<div class="fixed inset-0 z-50 bg-black/45" role="presentation" transition:fade={{ duration: 150 }} onclick={dismiss}></div>
		<div
			class="pb-safe fixed inset-x-0 bottom-0 z-50 rounded-t-3xl bg-bg shadow-float"
			role="menu"
			tabindex="-1"
			transition:fly={{ y: 320, duration: 220, opacity: 1 }}
		>
			<div class="mx-auto mt-2.5 h-1 w-10 rounded-full bg-fg/15"></div>
			{#if menu.header}
				<div class="flex items-center gap-3 border-b border-line px-5 pt-3 pb-3.5">
					{#if menu.header.art !== undefined}
						<Cover art={menu.header.art} class="size-11 shrink-0 rounded-md" />
					{/if}
					<div class="min-w-0">
						<div class="truncate font-semibold">{menu.header.title}</div>
						{#if menu.header.subtitle}<div class="truncate text-sm text-muted">{menu.header.subtitle}</div>{/if}
					</div>
				</div>
			{/if}
			<div class="p-2">
				{#each menu.items as item (item.label)}
					<button
						type="button"
						role="menuitem"
						class="flex h-12 w-full items-center gap-4 rounded-xl px-3 text-left text-[15px] font-medium active:bg-fg/6 {item.danger
							? 'text-accent'
							: ''}"
						onclick={() => menu.run(item)}
					>
						<item.icon class="size-5 {item.danger ? '' : 'text-muted'}" />
						{item.label}
					</button>
				{/each}
				<button
					type="button"
					class="mt-1 h-12 w-full rounded-xl bg-surface text-[15px] font-semibold active:bg-surface-2"
					onclick={() => menu.close()}
				>
					Cancel
				</button>
			</div>
		</div>
	{:else}
		<div
			class="fixed inset-0 z-50"
			role="presentation"
			onclick={dismiss}
			oncontextmenu={(e) => {
				e.preventDefault();
				menu.close();
			}}
		></div>
		<div
			role="menu"
			tabindex="-1"
			class="glass fixed z-50 rounded-2xl p-1.5 shadow-float ring-1 ring-line"
			style:width="{WIDTH}px"
			style:left="{left}px"
			style:top="{top}px"
			transition:fly={{ y: -4, duration: 120 }}
		>
			{#if menu.header}
				<div class="truncate px-3 pt-1.5 pb-2 text-xs font-semibold text-muted">
					{menu.header.title}{menu.header.subtitle ? ` · ${menu.header.subtitle}` : ''}
				</div>
			{/if}
			{#each menu.items as item (item.label)}
				<button
					type="button"
					role="menuitem"
					class="flex h-10 w-full items-center gap-3 rounded-xl px-3 text-left text-sm font-medium hover:bg-fg/6 {item.danger
						? 'text-accent'
						: ''}"
					onclick={() => menu.run(item)}
				>
					<item.icon class="size-4 {item.danger ? '' : 'text-muted'}" />
					{item.label}
				</button>
			{/each}
		</div>
	{/if}
{/if}
