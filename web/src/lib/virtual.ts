// Window-scrolled lists and grids that keep only the rows near the screen in the DOM (the albums'
// grid and list, the artists and genres): which rows those are, and where to scroll to open on an
// item. virtual.svelte.ts keeps a list's rows up to date with the page's scroll.

/**
 * The row at the top of the screen, with the list's top `scrolledPast` px above it (negative: below
 * it). In whole rows: scrolling within a row changes nothing, and there is nothing to redraw.
 */
export function topRow(scrolledPast: number, rowHeight: number): number {
	return rowHeight > 0 ? Math.floor(scrolledPast / rowHeight) : 0;
}

/** The rows to have in the DOM, from `first` up to `last` (exclusive): those in sight from `top`, and `overscan` more each way. */
export function rowWindow(top: number, viewport: number, rowHeight: number, rows: number, overscan: number): { first: number; last: number } {
	if (rowHeight <= 0) return { first: 0, last: 0 };
	return {
		first: Math.max(0, top - overscan),
		last: Math.max(0, Math.min(rows, Math.ceil(top + viewport / rowHeight) + overscan))
	};
}

/**
 * The page's scroll that puts row `row`'s item (`height` tall, at the top of its row) in the middle
 * of the screen, with the list's top `listTop` px down the page.
 */
export function revealScroll(listTop: number, row: number, rowHeight: number, height: number, viewport: number): number {
	return Math.max(0, listTop + row * rowHeight + height / 2 - viewport / 2);
}
