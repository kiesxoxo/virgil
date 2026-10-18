<script lang="ts">
	import GroupIndex from '#lib/components/GroupIndex.svelte';
	import { library } from '#lib/library.svelte.ts';
</script>

<GroupIndex kind="genre" groups={library.genres} title="Genres" />
