// Which album's cover moves between pages in the current view transition. Only that cover
// gets a view-transition-name: every named element is lifted out of the page and animated
// on its own, so naming all covers would flatten the cover flow and make phones stutter.

class Morph {
	album = $state<string | null>(null);

	/** The name for `album`'s cover in lists of albums: only the one that is moving has one. */
	name(album: string): string | undefined {
		return this.album === album ? `cover-${album}` : undefined;
	}
}

export const morph = new Morph();
