# audium — TODO

## growth
- asciinema recording on the README

### planned (priority order)
- **fields** (complexity 7/10) — let users attach author, album, and other
  custom fields to tracks. stored in library.json. not pulled from metadata —
  the user fills them in. prerequisite for filtering.
- **filtering** (complexity 5/10, after fields) — filter the tracklist by the
  custom fields added above.
- **playback speed** (complexity 5/10, after fields) — pre-set speed options
  (e.g. 0.5×, 0.75×, 1×, 1.25×, 1.5×, 2×) selectable from settings or a
  keybind. must be stable before lyrics land.
- **lyrics** (complexity 8/10) — time-synced lyrics added by the user,
  shown in a dedicated panel or overlay for the current track. highlight or
  typewriter animation. toggled with a keybind.
- **yt audio** (complexity 8.5/10) — pull audio from a youtube url directly
  into the library. on hold: a contributor is working on this.

### dev notes
- pop-up after filepicker might be buggy if the filename is too long. limit
chars so it doesn't overflow the pop-up char size.
- review the README and the web page before a the next release (v1.1.0 maybe?).
