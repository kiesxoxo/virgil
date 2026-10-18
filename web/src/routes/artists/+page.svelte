<script lang="ts">
	import GroupIndex from '#lib/components/GroupIndex.svelte';
	import { library } from '#lib/library.svelte.ts';
</script>

<GroupIndex kind="artist" groups={library.artists} title="Artists" />
