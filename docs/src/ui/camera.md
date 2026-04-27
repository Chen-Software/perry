# Camera

The `perry/ui` module provides a live camera preview widget with color
sampling capabilities.

```ts
{{#include ../../examples/ui/camera/snippets.ts:imports}}
```

> **Platform support:** real capture is implemented on **iOS**
> (AVCaptureSession) and **Android** (Camera2). On **macOS**, **Linux**
> (GTK4), **Windows**, and the **Web** target the runtime exports no-op
> stubs so cross-platform code compiles and links cleanly — `CameraView()`
> returns handle 0 and `cameraSampleColor` returns `-1`. Wiring real
> capture on those platforms (AVFoundation on macOS, GStreamer/V4L2 on
> Linux, Media Foundation on Windows, `getUserMedia` on Web) is tracked as
> a follow-up.

## Quick Example

```ts
{{#include ../../examples/ui/camera/snippets.ts:quick-example}}
```

## API Reference

### `CameraView()`

Create a live camera preview widget.

```ts
{{#include ../../examples/ui/camera/snippets.ts:camera-create}}
```

Returns a widget handle. The camera does not start automatically — call `cameraStart()` to begin capture.

### `cameraStart(handle)`

Start the live camera feed.

```ts
{{#include ../../examples/ui/camera/snippets.ts:camera-start}}
```

On iOS, the camera permission dialog is shown automatically on first use.

### `cameraStop(handle)`

Stop the camera feed and release the capture session.

```ts
{{#include ../../examples/ui/camera/snippets.ts:camera-stop}}
```

### `cameraFreeze(handle)`

Pause the live preview (freeze the current frame).

```ts
{{#include ../../examples/ui/camera/snippets.ts:camera-freeze}}
```

The camera session remains active but the preview stops updating. Useful for "capture" moments where you want to inspect the frozen frame.

### `cameraUnfreeze(handle)`

Resume the live preview after a freeze.

```ts
{{#include ../../examples/ui/camera/snippets.ts:camera-unfreeze}}
```

### `cameraSampleColor(x, y)`

Sample the pixel color at normalized coordinates.

```ts
{{#include ../../examples/ui/camera/snippets.ts:sample-color}}
```

- `x`, `y` are normalized coordinates (0.0–1.0)
- Returns packed RGB as a number: `r * 65536 + g * 256 + b`
- Returns `-1` if no frame is available

To extract individual channels:

```ts
{{#include ../../examples/ui/camera/snippets.ts:extract-channels}}
```

The color is averaged over a 5x5 pixel region around the sample point for noise reduction.

### `cameraSetOnTap(handle, callback)`

Register a tap handler on the camera view.

```ts
{{#include ../../examples/ui/camera/snippets.ts:on-tap}}
```

The callback receives normalized coordinates of the tap location, which can be passed directly to `cameraSampleColor()`.

## Implementation

On iOS, the camera uses AVCaptureSession with AVCaptureVideoPreviewLayer for GPU-accelerated live preview, and AVCaptureVideoDataOutput for frame capture. Color sampling reads pixel data from CVPixelBuffer.

On Android, the camera uses Camera2 with a TextureView preview surface. Color sampling reads from the most recent ImageReader frame.

## Next Steps

- [Widgets](widgets.md) — All available widgets
- [Audio Capture](../system/audio.md) — Microphone input and sound metering
