# embpic

Minimal `no_std` image buffer for embedded. JPEG load/save.

```rust
use embpic::{denoise::denoisers, filter::filters, resize::resizers, Color, Image};

let mut img = Image::new(320, 240);
img.put_pixel(10, 20, Color::rgb(255, 0, 0));

let img = Image::load("photo.jpg")?;
img.save("out.jpg")?;

let big = img.resize(&resizers::bilinear(1080, 1920));
let fast = img.resize(&resizers::nearest(160, 120));

let clean = img.denoise(&denoisers::bilateral(2, 1.5, 25.0));

let soft = img.filter(&filters::gaussian(1.5));
let gray = img.filter(&filters::grayscale());
let punch = img.filter(&filters::high_contrast(1.8));
```

## API

| Item | Note |
|---|---|
| `Image::new(w, h)` | black RGB buffer (`alloc::vec`) |
| `Image::from_rgb(w, h, buf)` | wrap raw RGB bytes |
| `Image::load(path)` | `.jpg` / `.jpeg`, via mmap syscalls |
| `img.save(path)` | baseline JPEG encoder (q50, 4:4:4) |
| `img.resize(&resizers::bilinear(w, h))` | smooth (default choice) |
| `img.resize(&resizers::nearest(w, h))` | fast, blocky |
| `img.denoise(&denoisers::mean(r))` | box blur, cheap |
| `img.denoise(&denoisers::median(r))` | kills salt-and-pepper |
| `img.denoise(&denoisers::bilateral(r, ss, sc))` | edge-preserving |
| `img.filter(&filters::gaussian(s))` | separable blur, `s<=0` = copy |
| `img.filter(&filters::grayscale())` | Rec.601 luma, stays RGB |
| `img.filter(&filters::high_contrast(f))` | stretch around mid-gray |
| `img.normalize()` | per-channel min-max to 0–255 |
| `put_pixel` / `get_pixel` | OOB put = noop, OOB get = `None` |
| `Color::rgb(r, g, b)` | + `BLACK WHITE RED GREEN BLUE` |

## Platform

- `no_std` + `alloc` (bring your own allocator on MCU).
- File IO uses raw Linux x86_64 syscalls (`src/internal/platform`), no libc.
- Other targets: codec compiles, `load`/`save` return `UnsupportedPlatform`.

## Limits

- JPEG baseline 8-bit only (no progressive)..
- Encoder: fixed quality (~q50), no subsampling.
- `resize` uses plain `f32` arithmetic only.
