# Brand

| File | Use |
|---|---|
| `icon.svg` | Source of the logo and app icon. Linux packages install it as the `revisor` icon. |
| `icon-1024.png` | Rendered from `icon.svg`. `scripts/package-macos.sh` makes `Revisor.icns` from it. |
| `social-preview.png` | GitHub repository social preview and link card (1280x640). Source: `social-preview.html`. |

The icon shows a review comment that lands on a diff line. Colors come from the Revisor Islands theme in `crates/revisor-app/src/theme.json`:

| Color | Hex |
|---|---|
| Background | `#121216` to `#22232a` |
| Comment (primary) | `#548af7` |
| Added line | `#5fb865` |
| Removed line | `#f75464` |
| Context line | `#4a4c55` |
| Gutter | `#bcbec4` |

Tagline: **AI code review that lands on the diff.**

## Render the PNG files

After you change `icon.svg` or `social-preview.html`, render the PNG files again with Chrome:

```sh
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
printf '<body style="margin:0"><img src="icon.svg" width="1024" style="display:block">' > brand/icon.html
"$chrome" --headless --allow-file-access-from-files --default-background-color=00000000 --force-device-scale-factor=1 \
    --window-size=1024,1024 --screenshot=brand/icon-1024.png "file://$PWD/brand/icon.html"
rm brand/icon.html
"$chrome" --headless --allow-file-access-from-files --force-device-scale-factor=1 \
    --window-size=1280,640 --screenshot=brand/social-preview.png "file://$PWD/brand/social-preview.html"
```
