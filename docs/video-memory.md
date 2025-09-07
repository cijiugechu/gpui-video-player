# Video memory management and leak fix

This document explains the root cause of the large memory growth when playing video, the fixes applied, why macOS CoreVideo is used, and where `unsafe` is required. It also discusses alternatives such as bytemuck/zerocopy.

## Symptom

- While playing a short video, process memory (especially IOAccelerator/graphics) grew from hundreds of MB to multiple GB.
- `leaks` showed tiny malloc leaks, but `vmmap -summary` revealed growth in IOAccelerator (graphics), not in malloc. This pointed to GPU texture accumulation rather than heap allocations.

## Root cause

- The `Element` rendering path created a new `gpui::RenderImage` every frame and painted it with `window.paint_image(...)`.
- Each new `RenderImage` has a fresh `ImageId`. GPUI caches image textures in the sprite atlas keyed by `ImageId`.
- Without explicitly dropping old textures, the atlas kept every frame’s texture resident → unbounded GPU memory growth in IOAccelerator.

## Fixes

### 1) Explicitly drop previous `RenderImage` each frame (cross‑platform)

- `VideoElement` stores the last `Arc<RenderImage>` and, after painting the new frame, drops the previous one via `cx.drop_image(prev, Some(window))`.
- This evicts the texture from the sprite atlas on all windows, preventing accumulation.
- Result: IOAccelerator memory stays bounded and stable when rendering via the sprite atlas.

Why this is necessary: GPUI treats each `RenderImage` ID as a distinct atlas entry. Generating a new image per frame requires manual eviction to avoid retaining all older textures.

### 2) macOS: render NV12 via CoreVideo `CVPixelBuffer` + `window.paint_surface`

- On macOS, we build a `CVPixelBuffer` with pixel format NV12 and call `window.paint_surface(...)`, bypassing the sprite atlas entirely.
- This path copies the current frame’s two NV12 planes into a `CVPixelBuffer` that is Metal‑compatible and backed by an IOSurface.
- GPUI’s Metal renderer samples the planes directly, avoiding per‑frame atlas uploads.
- Result: GPU memory usage remains low and stable and does not grow over time.

Rationale: `paint_surface` is the intended path in GPUI for dynamic video surfaces on macOS; it avoids atlas management for streaming content.

## Are the appsink pipeline tweaks still useful?

Yes. We keep these safeguards:
- `drop=true`, `max-buffers=3`, `enable-last-sample=false` on the appsink.
- Enforced both in the `playbin` pipeline string and the custom‑pipeline constructor.

These do not fix the GPU leak (which was in the UI layer) but they:
- Bound appsink’s internal queue to avoid hidden buffering/latency.
- Reduce peak memory and avoid pressure if the UI stalls.
- Make behavior more predictable.

## Why CoreVideo and where `unsafe` is used

We use CoreVideo on macOS to construct a `CVPixelBuffer` from the current NV12 frame and feed it to `window.paint_surface(...)`. This integrates with GPUI’s Metal path (IOSurface + CoreVideo texture cache).

Unavoidable `unsafe` sites and why they are contained:
- Mapping `CVPixelBuffer` planes exposes raw pointers. Copying bytes into those planes uses `std::ptr::copy_nonoverlapping`.
  - There is no safe Rust API that hands out plane `&mut [u8]` for CoreVideo. Creating a slice from the base pointer would itself require `unsafe`.
  - We keep `unsafe` minimal: compute plane sizes from NV12, lock/unlock the pixel buffer, and copy exactly `y_size` and `uv_size` bytes.
- CoreFoundation dictionary building for `CVPixelBuffer` attributes (Metal‑compatible, IOSurface‑backed) is done via the safe wrappers in `core-foundation`.

We intentionally avoided a “zero‑copy with release callback” path (`CVPixelBufferCreateWithPlanarBytes`) because:
- It requires an `extern "C"` release callback and careful lifetime management of the backing allocation.
- Errors in attributes or lifetime can cause rendering failures (we observed error code −6660 from the Metal texture cache when experimenting).
- The chosen implementation uses `CVPixelBuffer::new(...)` + `lock_base_address` → simpler and robust.

## Can bytemuck or zerocopy remove the `unsafe`?

- bytemuck/zerocopy help with safe reinterpretation of in‑process memory, but they do not eliminate `unsafe` when dealing with FFI pointers returned by CoreVideo.
- To write into a `CVPixelBuffer` plane you must:
  1) Lock the buffer.
  2) Obtain a raw base address from CoreVideo (FFI pointer).
  3) Copy bytes into that memory.
- Steps (2) and (3) fundamentally involve `unsafe`. You can wrap the raw pointer into `&mut [u8]` and then use `copy_from_slice`, but creating that slice is still `unsafe`.

In short: bytemuck/zerocopy do not remove the FFI pointer unsafety here. Our code confines `unsafe` to the minimal, audited region (two copies into known‑size NV12 planes).

## Future refinements

- Pool `CVPixelBuffer`s with `CVPixelBufferPool` to reduce allocations and locks under load.
- If desired, gate the CoreVideo path behind a Cargo feature to allow falling back to atlas (already safe with explicit drop).
- On non‑macOS, continue the atlas path with explicit eviction.

## Summary

- The leak was caused by accumulating per‑frame textures in the GPUI sprite atlas.
- Fix: drop last `RenderImage` each frame, or use `paint_surface` on macOS with `CVPixelBuffer` to avoid the atlas.
- Appsink queue limits remain beneficial guards but were not the root cause.
- `unsafe` is limited to copying bytes into a `CVPixelBuffer`’s planes; bytemuck/zerocopy cannot remove this FFI pointer unsafety.
