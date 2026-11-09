<script lang="ts">
	// A QR code, black on white in both themes (cameras want dark on light), square modules.
	import { encode } from 'uqr';

	let { text, class: cls = '' }: { text: string; class?: string } = $props();

	let qr = $derived(encode(text, { ecc: 'M', border: 2 }));
	let d = $derived(qr.data.flatMap((row, y) => row.map((on, x) => (on ? `M${x} ${y}h1v1h-1z` : ''))).join(''));
</script>

<svg viewBox="0 0 {qr.size} {qr.size}" class="block bg-white {cls}" shape-rendering="crispEdges" role="img" aria-label="QR code for {text}">
	<path {d} fill="#000" />
</svg>
