<script lang="ts">
	import { page } from '$app/state';
	import GroupPage from '#lib/components/GroupPage.svelte';
	import { appearsOn, find } from '#lib/browse.ts';
	import { library } from '#lib/library.svelte.ts';

	let name = $derived(page.params.name ?? '');
	let group = $derived(find(library.artists, name));
	let collaborations = $derived(group ? appearsOn(library.entries, group.key) : []);
</script>

<GroupPage kind="artist" {group} {name} appearsOn={collaborations} />
