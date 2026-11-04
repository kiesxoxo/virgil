<script lang="ts">
	// The transport's symbols, drawn flat and square.
	const paths = {
		play: 'M7 4l14 8-14 8z',
		pause: 'M6 4h4.5v16H6zM13.5 4H18v16h-4.5z',
		next: 'M4 4l13 8-13 8zM17.5 4H20v16h-2.5z',
		previous: 'M20 4L7 12l13 8zM4 4h2.5v16H4z'
	};

	let { name, class: className = '' }: { name: keyof typeof paths; class?: string } = $props();
</script>

<svg viewBox="0 0 24 24" class="fill-current {className}" aria-hidden="true"><path d={paths[name]} /></svg>
