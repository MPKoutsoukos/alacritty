# Alacritty with background images

A fork of [Alacritty](https://github.com/alacritty/alacritty) that adds one
option, a picture drawn behind the terminal text:

```toml
[window.background_image]
path = 'C:\path\to\image.png'   # PNG only
position = "Center"             # TopLeft, Top, TopRight, Left, Center, Right,
                                # BottomLeft, Bottom or BottomRight
opacity = 1.0
```

The image keeps its own size, shrinks to fit a smaller window, and works with
`window.opacity`. Everything else is plain Alacritty.

This is not part of Alacritty and isn't going to be: its maintainers have
decided against background images. Please don't report problems with this
build to them.

## Downloads

Windows builds are on the [releases page](https://github.com/MPKoutsoukos/alacritty/releases): an `.msi`
installer and a portable `.exe`. The installer replaces an installed
Alacritty of the same version.

## How it stays up to date

- `background-image` holds the change: a few commits on top of an Alacritty
  release.
- `pipeline` (this branch) holds the build. Once a day it checks for a new
  Alacritty release. When there is one, or `background-image` has changed, it
  moves the change onto the newest release, runs the tests, builds the exe
  and the installer, and publishes them as `vX.Y.Z-bg`.
- If the change stops applying cleanly to a new release, the run fails and
  GitHub sends an email. Rebase `background-image` onto the new release by
  hand, push it, and the next run picks it up (or start one from the
  Actions tab).

Licensed like Alacritty, under Apache 2.0 or MIT.
