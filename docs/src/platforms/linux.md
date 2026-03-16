# Linux (GTK4)

Perry compiles TypeScript apps for Linux using GTK4.

## Requirements

- GTK4 development libraries:
  ```bash
  # Ubuntu/Debian
  sudo apt install libgtk-4-dev

  # Fedora
  sudo dnf install gtk4-devel

  # Arch
  sudo pacman -S gtk4
  ```
- Cairo development libraries (for canvas):
  ```bash
  sudo apt install libcairo2-dev
  ```

## Building

```bash
perry app.ts -o app --target linux
./app
```

## UI Toolkit

Perry maps UI widgets to GTK4 widgets:

| Perry Widget | GTK4 Widget |
|-------------|------------|
| Text | GtkLabel |
| Button | GtkButton |
| TextField | GtkEntry |
| SecureField | GtkPasswordEntry |
| Toggle | GtkSwitch |
| Slider | GtkScale |
| Picker | GtkDropDown |
| ProgressView | GtkProgressBar |
| Image | GtkImage |
| VStack | GtkBox (vertical) |
| HStack | GtkBox (horizontal) |
| ZStack | GtkOverlay |
| ScrollView | GtkScrolledWindow |
| Canvas | Cairo drawing |
| NavigationStack | GtkStack |

## Linux-Specific APIs

- **Menu bar**: GMenu / set_menubar
- **Toolbar**: GtkHeaderBar
- **Dark mode**: GTK settings detection
- **Preferences**: GSettings or file-based
- **Keychain**: libsecret
- **Notifications**: GNotification
- **File dialogs**: GtkFileChooserDialog
- **Alerts**: GtkMessageDialog

## Styling

GTK4 styling uses CSS under the hood. Perry's styling methods (colors, fonts, corner radius) are translated to CSS properties applied via `CssProvider`.

## Testing with Geisterhand

Perry's built-in UI fuzzer works on Linux/GTK4. Screenshots use `WidgetPaintable` + `GskRenderer` for pixel-accurate capture.

```bash
perry app.ts -o app --target linux --enable-geisterhand
./app
# In another terminal:
curl http://127.0.0.1:7676/widgets
curl http://127.0.0.1:7676/screenshot -o screenshot.png
```

See [Geisterhand](../testing/geisterhand.md) for full API reference.

## Next Steps

- [Platform Overview](overview.md) — All platforms
- [UI Overview](../ui/overview.md) — UI system
