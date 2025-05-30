<h1 align="center" style="display: block; font-size: 2.5em; font-weight: bold; margin-block-start: 1em; margin-block-end: 1em;">
  <img align="center" src="https://raw.githubusercontent.com/monoamine11231/MeowPDF/refs/heads/master/assets/logo.png" alt="MeowPDF Logo" style="width:100%;height:100%"/><br />
  <br/>
  <strong>MeowPDF</strong><br/>
  <img align="center" src="https://github.com/monoamine11231/MeowPDF/actions/workflows/build.yml/badge.svg"/>
</h1>

*A Kitty terminal PDF viewer with Vim-like keybindings and classical GUI-like usage.*

[![Latest release](https://img.shields.io/github/v/release/monoamine11231/meowpdf?label=Latest%20release&style=social)](https://github.com/monoamine11231/meowpdf/releases/tag/v1.2.0)
[![GitHub commits](https://img.shields.io/github/commits-since/monoamine11231/meowpdf/v1.2.0.svg?style=social)](https://GitHub.com/monoamine11231/meowpdf/commit/)
[![Stars](https://img.shields.io/github/stars/monoamine11231/meowpdf?style=social)](https://github.com/monoamine11231/meowpdf/stargazers)
[![Fork](https://img.shields.io/github/forks/monoamine11231/meowpdf?style=social)](https://github.com/monoamine11231/meowpdf/network/members)
[![Watchers](https://img.shields.io/github/watchers/monoamine11231/meowpdf?style=social)](https://github.com/monoamine11231/meowpdf/watchers)

<hr/>

<img src="https://raw.githubusercontent.com/monoamine11231/MeowPDF/refs/heads/master/assets/overview.gif"></img>
*Note that the colours in the overview above are corrupted because of GIF compression.* 
<hr/>

## Why?
There are multiple in-terminal PDF viewers for the Kitty terminal but the main problem is that the end-user can interact only by viewing one page at a time. The user may have the need to zoom in and out of the PDF document to view details in the document. Another problem is viewing continuous content which is split between multiple pages. Therefore it was decided to develop such PDF viewer which can operates in the same way as a classical GUI PDF viewer but which can additionally be controlled by powerful Vim-like keybindings.

<hr/>

## Table of Contents
- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
  - [Configuration](#configuration)
    - [Keybindings](#keybindings)
    - [URI Bar](#uri-bar)
- [TODO](#todo)
- [Contributions](#contributions)
- [License](#license)
- [Attributions](#attributions)

<hr/>

## Requirements
- Cargo
- Rust
- Kitty >= 0.31.0

<div align="right"><kbd><a href="#table-of-contents">↑ Back to top ↑</a></kbd></div>
<hr/>

## Installation
The project is easily built and installed using Cargo:
```sh
$ cargo build --release && cargo install -path .
```

<div align="right"><kbd><a href="#table-of-contents">↑ Back to top ↑</a></kbd></div>
<hr/>

## Usage
To view a PDF file simply execute:
```sh
$ meowpdf <PATH TO PDF FILE>
```

### Configuration
One of the key-features of *MeowPDF* is it's high customizability. *MeowPDF* allows customization based on the following parameters:
- Scroll speed
- Static render precision for PDF pages
- Memory limit on rendered PDF pages
- Default document scale on enter (will be replaced by a dynamic one soon)
- Minimal allowed zoom out amount on the document
- Zoom amount
- Margin amount on the bottom of PDF pages
- Preloaded pages before and after the first displayed page
- Keybindings
- URI annotation bar

The configuration TOML file is found in `~/.config/meowpdf`.

#### Keybindings
The default keybindings are listed bellow:
- **q/Q**: Quit
- **a/A**: Toggles alpha on PDF pages (Makes white background of PDF pages transparent)
- **i/I**: Toggles color inversion on PDF pages
- **c/C**: Center the viewer
- **gg**: Jumps to the first page of the PDF document
- **G**: Jumps to the last page of the PDF document
- **\<left\>**: Move the document to the left
- **\<right\>**: Move the document to the right
- **\<up\>**: Move the pages up (& the document down)
- **\<down\>**: Move the pages down (& the document up)
- **+**: Zoom in
- **-**: Zoom out

The keybindings can be customized by modifying the `[bindings]` section in the configuration file. The syntax for expressing key combinations is the same as of [keybinds-rs](https://github.com/rhysd/keybinds-rs/blob/main/doc/binding_syntax.md). The actions that keys can be bound to are the following:
- `ToggleAlpha`: Toggles the alpha color mode.
- `ToggleInverse`: Toggles the inverse color mode.
- `CenterViewer`: Centers the viewer.
- `ZoomIn`: Zooms in the viewer.
- `ZoomOut`: Zooms out the viewer.
- `JumpFirstPage`: Jumps to the first page of the document.
- `JumpLastPage`: Jumps to the last page of the document.
- `Quit`: Quits the document.

> [!WARNING]
> Be aware that character keys such as `a`, `b`, ... can not be combined with the Shift modifier explicitely. Capitalize the characters instead.

#### URI Bar
The URI annotation bar shows up when hovering a mouse event hovers a link in the PDF document. That bar displays the path of the hovered link.

The URI bar can be customized by modifying the `[viewer.uri_hint]` section in the configuration file. The following parameters can be set and modified:
- `enabled` (`true/false`): Enables the URI annotation bar.
- `background` (`string`): Sets the background color of the URI annotation bar.
- `foreground` (`string`): Sets the foreground color of the URI annotation bar.
- `width` (`f32`): Sets the maximum width of the bar as a factor based off the current terminal size.

The allowed color strings are listed [here](https://docs.rs/crossterm/latest/src/crossterm/style/types/color.rs.html#221-259).

> [!NOTE]
> The minimal URI bar width is currently 5 column cells.

<div align="right"><kbd><a href="#table-of-contents">↑ Back to top ↑</a></kbd></div>
<hr/>

## TODO
### Future ideas
- [x] Remove heavy and inefficient regex dependency and move to nested switches.
- [x] Implement or find a standard on parsing stdin key inputs.
- [x] Allow for link clicking using the mouse.
- [x] Allow custom remapping of keybindings.

### In progress
- [x] Implement auto-scaling of the PDF document on opening based on terminal size.
- [ ] Develop a customizable Vim-like bar illustrating page & document metrics.


<div align="right"><kbd><a href="#table-of-contents">↑ Back to top ↑</a></kbd></div>
<hr/>

## Contributions
All contributions are welcome to this project. 

<div align="right"><kbd><a href="#table-of-contents">↑ Back to top ↑</a></kbd></div>
<hr/>

## License
*MeowPDF* is available under [MIT](https://github.com/monoamine11231/MeowPDF/blob/master/LICENSE).

<div align="right"><kbd><a href="#table-of-contents">↑ Back to top ↑</a></kbd></div>
<hr/>

## Attributions
1. Logo was made using <a href="https://www.vecteezy.com/free-vector/peeking-cat">Peeking Cat Vectors by Vecteezy</a>.
2. README design was inspired from [areg-sdk](https://github.com/aregtech/areg-sdk).
<div align="right"><kbd><a href="#table-of-contents">↑ Back to top ↑</a></kbd></div>
