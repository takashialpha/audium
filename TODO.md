dev (notes):
- try making audium popular: awesome lists like ratatui or tui, asciinema on readme,
compare with termusic, more marketing stuff.
(rethinking on ux)
- Add a menu instead of settings directly; that menu gives access to settings and a fancy about screen.
- Review bindings to focus on a user-friendlier one.

bugs to solve:
- None yet!

features to add:
- 1 (complexity: 8/10) add time-synced lyrics, with highlighting
or "typewriter" animation(might me more complicated), using a bind for them, and
they always show it for the current playing song. the lyrics are supposed to be added by
the user;
- 2 (complexity: 7/10) add fields like author/album..more that are meant to be filled by the user,
not by metadata, this way it's possible to filter by it without
breaking the philosophy of "it's your library" (gonna affect library.json); after that, add filtering by the fields added in the previous feature;
- 3 (complexity: 8.5/10) add the possibility of pulling audio from yt videos;
