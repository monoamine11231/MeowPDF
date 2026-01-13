## [1.2.2] - 2026-01-13

### Added

- Next and previous page jump commands with corresponding keybindings ([@PhilT](https://github.com/PhilT)).
- Auto-fix of the config file when new keybindings are implemented ([#9](https://github.com/monoamine11231/MeowPDF/issues/9)).

### Fixed

- Inverting scroll direction now applies on mouse scroll wheel only and not on keys ([@PhilT](https://github.com/PhilT)).

## [1.2.1] - 2025-06-10

### Added

- Basic CLI commands such as help + version [#7](https://github.com/monoamine11231/MeowPDF/issues/7).
- `hjkl` keybindings for cursor control [#7](https://github.com/monoamine11231/MeowPDF/issues/7).
- Vertical scroll inversion configuration key.

### Changed

- Vertical scrolling has been made non-inverse by default.

## [1.2.0] - 2025-05-30

### Added

- Link clicking and hovering.
- URI annotation bar showing the hovered link path.
- Better documentation on the customization of different components.

### Removed

- Type hints from the code base.

### Changed

- The messy display method by now using display rectangular boundaries.

## [1.1.0] - 2025-05-28

### Added

- Customization of keyboard bindings by integration of `keybinds-rs`.
- Automatic scaling of page size to terminal size on start.
- Priority channels for less boilerplate on inter-thread communication.
- Integrated `crossterm` fork to support Kitty terminal graphics responses and more.

### Removed

- Default scale of the document when entering a document.
- Window resize thread as it is handled by `crossterm`.
- `broadcast.rs` as it is replaced by `priority_channel.rs`.
- Bar descriptions from config as a complete rewrite is planned soon.
- `tui.rs` as nearly all of the functions are implemented in `crossterm` crate.
- Manual parsing of events from stdin using regex DFA's.

### Changed

- Separated `viewer.rs` into a separate thread module for handling rendering events.
- Separated threads from `main.rs` into separate modules. 
- Modified the thread communication between threads so that every event goes through `fn main`.
