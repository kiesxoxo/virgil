<script lang="ts">
	import { LibraryBig, MicVocal, Shapes } from '@lucide/svelte';
	import { page } from '$app/state';

	// Phones: a compact switch between the library's views, beside the other
	// view controls. Desktop has them in the sidebar. Each reaches 44px tall for a finger.
	const views = [
		{ href: '/', label: 'Albums', icon: LibraryBig },
		{ href: '/artists', label: 'Artists', icon: MicVocal },
		{ href: '/genres', label: 'Genres', icon: Shapes }
	];
</script>

<nav class="flex rounded-lg bg-surface p-0.5 lg:hidden" aria-label="Library">
	{#each views as view (view.href)}
		{@const active = page.url.pathname === view.href}
		<a
			href={view.href}
			class="relative after:absolute after:inset-x-0 after:-inset-y-2 grid size-7 place-items-center rounded-md {active
				? 'bg-bg text-fg shadow-sm ring-1 ring-line'
				: 'text-muted hover:text-fg'}"
			aria-current={active ? 'page' : undefined}
			aria-label={view.label}
			title={view.label}
		>
			<view.icon class="size-4" />
		</a>
	{/each}
</nav>
