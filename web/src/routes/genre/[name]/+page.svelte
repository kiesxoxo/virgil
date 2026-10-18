<script lang="ts">
	import { page } from '$app/state';
	import GroupPage from '#lib/components/GroupPage.svelte';
	import { find } from '#lib/browse.ts';
	import { library } from '#lib/library.svelte.ts';

	let name = $derived(page.params.name ?? '');
	let group = $derived(find(library.genres, name));
</script>

<GroupPage kind="genre" {group} {name} />
