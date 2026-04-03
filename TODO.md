dev notes:
- fix 3 clippy warnings and fix them (all allow dead_code or unused.. were removed, update all codebase);
- try showing non-fatal errors in pop-ups instead of eprintln!'s since it's a ratatui app;
- complete the above and feature no. 5 before v1 (stable);

bugs to solve:
- None yet!

features to add:
- 1 (complexity: 8/10) add time-synced lyrics, with highlighting
or "typewriter" animation(might me more complicated), using a bind for them, and
they always show it for the current playing song. the lyrics are supposed to be added by
the user;
- 2 (complexity: 7/10) add fields like author/album..more that are meant to be filled by the user,
not by metadata, this way it's possible to filter by it without
breaking the philosophy of "it's your library" (should affect library.json);
- 3 (complexity: 5/10; after 2) add filtering by the fields added in the previous feature;
- 4 (complexity: 8.5/10) add the possibility of pulling audio from yt videos;
- 5 (complexity: 5/10) add modes like loop/not looping, so at the end of the queue songs
can repeat;

docs:
- add things saying that "it's your library";
- make clear that it only outputs via the main system output because changing the output in the
system itself is cleaner and works better;
- try making it fancier and compare both (as audium is lighter, better ui, less deps/
smaller build time, more modern, and more);
