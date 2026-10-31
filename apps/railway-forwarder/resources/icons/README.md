# App icon

`AppIcon.png` is the authoritative 1024-pixel app icon. Its material rendering
was redesigned against the installed macOS 27 Shortcuts, Passwords, Settings,
Mail and Podcasts icons. The design uses a restrained violet enclosure,
centered overhead light, frosted circular glass plates and crisp reflective
edges. The transparent margin follows the macOS app-icon proportions.

`MenuSymbol.svg` preserves the exact AppKit-exported silhouette of
`point.3.filled.connected.trianglepath.dotted` as the geometry reference. The
new rendering preserves its three large nodes and seven connecting dots.
`GENERATION.md` records the built-in image generation prompt and references.

`AppIcon.icns` contains the standard macOS icon sizes and Retina
representations. This is a rendered glass-style icon, rather than an adaptive
Icon Composer asset.

After updating the PNG, regenerate the ICNS using Apple's built-in tools:

```sh
bash scripts/build-icon.sh
```

The regular build copies `AppIcon.icns` into the bundle's `Contents/Resources`
and declares it with `CFBundleIconFile`. Builds do not require an image renderer
or a full Xcode installation.
