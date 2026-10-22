# Desk SDK

The C ABI that `DESKCORE.EXE` exports to apps. Every function is `extern "C"`, cdecl, and takes and returns only integers, pointers and lengths. Handles are `u32`; `0` is never a valid handle. Strings are UTF-8 with a length, never NUL-terminated across the boundary. Rust apps use the `desk-sdk` crate, which wraps these in safe types.

## An app's entry points

An app's `APP.DXE` exports three symbols:

```c
int  app_start(u32 app, const char *args, u32 args_len);   /* 0 = started */
void app_event(u32 app, const DeskEvent *e);               /* one event at a time */
void app_stop(u32 app);                                    /* free everything */
```

`DeskEvent` is the one struct shared by pointer (never by value): `{ u32 kind; u32 window; u32 control; i32 x; i32 y; u32 key; u32 mods; u32 data; }`. Kinds: `open_file`, `close`, `paint`, `mouse_down`, `mouse_up`, `mouse_move`, `key`, `char`, `menu`, `control`, `timer`, `resize`, `focus`, `blur`, `clipboard`, `settings_changed`.

## Windows

```c
u32  desk_window_new(u32 app, const char *title, u32 len, i32 w, i32 h, u32 flags);
void desk_window_title(u32 win, const char *title, u32 len);
void desk_window_show(u32 win);
void desk_window_close(u32 win);
void desk_window_invalidate(u32 win, i32 x, i32 y, i32 w, i32 h);
```

Flags: `resizable`, `dialog` (modal to its owner), `tool` (no button on the bar), `remember` (position kept in `DESK.INI`).

## Drawing

Apps draw only inside a `paint` event, into the window's client area, through the Desk. The Desk clips and tracks dirty rectangles.

```c
void desk_fill(u32 win, i32 x, i32 y, i32 w, i32 h, u32 role);
void desk_line(u32 win, i32 x0, i32 y0, i32 x1, i32 y1, u32 role);
void desk_text(u32 win, i32 x, i32 y, const char *s, u32 len, u32 font, u32 role);
i32  desk_text_width(const char *s, u32 len, u32 font);
void desk_blit(u32 win, i32 x, i32 y, const u8 *pixels, i32 w, i32 h, i32 stride);
void desk_icon(u32 win, i32 x, i32 y, u32 icon, u32 role);
```

`role` is a **colour role** (`text`, `text_muted`, `surface`, `surface_raised`, `accent`, `selection`, `success`, `warning`, `error`, …), not a palette index, so the same app draws correctly in both looks and both colour depths. `desk_blit` takes palette indices; apps that need their own colours (Paint) ask for the lent range with `desk_palette_lend`.

## Widgets, menus and dialogs

```c
u32  desk_control(u32 win, u32 kind, i32 x, i32 y, i32 w, i32 h, const char *label, u32 len);
void desk_control_set(u32 control, u32 property, i32 value);
i32  desk_control_get(u32 control, u32 property);
u32  desk_menu_new(u32 win);
void desk_menu_add(u32 menu, u32 id, const char *label, u32 len, u32 key, u32 flags);
i32  desk_ask(u32 win, const char *title, u32 tlen, const char *text, u32 len, u32 buttons, u32 default_button);
```

Control kinds: `button`, `checkbox`, `radio`, `text_field`, `text_area`, `list`, `slider`, `progress`, `label`, `scrollbar`. `desk_ask` is the only modal: destructive questions pass `default_button = no`.

## Menu bar, toolbar, tabs and status bar

The shared chrome from `HIG.md`. The Desk draws and lays them out; the app only describes them and gets `menu` and `control` events back.

```c
u32  desk_menubar(u32 win);                                       /* one per window, under the title bar */
u32  desk_menubar_menu(u32 bar, const char *label, u32 len);       /* File, Edit, View, …, Help */
void desk_menu_check(u32 menu, u32 id, i32 on);
void desk_menu_enable(u32 menu, u32 id, i32 on);
u32  desk_toolbar(u32 win);
void desk_toolbar_add(u32 bar, u32 id, u32 glyph, const char *label, u32 len, u32 flags);   /* flags: toggle, separator_before, push_right */
void desk_toolbar_state(u32 bar, u32 id, i32 on, i32 enabled);
u32  desk_tabs(u32 win, u32 flags);                               /* flags: documents (close + new) or sections */
u32  desk_tab_add(u32 tabs, const char *label, u32 len, u32 icon);
void desk_tab_set(u32 tab, u32 property, i32 value);              /* active, unsaved */
void desk_status(u32 win, const char *left, u32 llen, const char *right, u32 rlen);
void desk_about(u32 app);                                         /* About box from APP.INF */
```

Events: `menu` (data = item id), `control` (toolbar id), `tab_select`, `tab_close`, `tab_new`.

## Context menus

```c
u32  desk_popup(u32 win, i32 x, i32 y);                           /* at the pointer, from a right-click */
void desk_popup_add(u32 popup, u32 id, const char *label, u32 len, u32 key, u32 flags); /* flags: default (bold), check, disabled, separator_before */
void desk_popup_show(u32 popup);
```

The app answers the `context` event (x, y, and the control under the pointer). The default action comes first, Properties last.

## Pickers

```c
i32 desk_pick_open(u32 win, const char *types, u32 tlen, char *out, u32 out_len);
i32 desk_pick_save(u32 win, const char *name, u32 nlen, char *out, u32 out_len);
i32 desk_pick_folder(u32 win, char *out, u32 out_len);
```

Return the length of the chosen path, or `0` if cancelled. `types` is `"MID;RMI"`, or several groups: `"Text|TXT;MD;INI"`. The picker is the shared one built from Files: places and drives on the left, a type filter, an 8.3 name field, and a confirm before replacing a file (defaulting to keep it). `desk_pick_places(win, "C:\\PAINT;C:\\DESK\\WALLPAPR")` adds an app's own places for the next pick.

## Files dropped on a window

```c
void desk_accept_drops(u32 win, const char *types, u32 len);      /* which extensions this window takes */
```

A drop arrives as `open_file` events with `data = 1` (dropped, not opened), one per file. Notes opens tabs, Audio Player queues, Paint opens pictures. The Desk shows "→ {App}" on the drag ghost only over windows that accept the type.

## Files

```c
i32 desk_file_read(const char *path, u32 plen, u8 *out, u32 out_len);
i32 desk_file_write(const char *path, u32 plen, const u8 *data, u32 len);   /* creates or replaces; marks the archive bit */
i32 desk_file_unique(const char *dir, u32 dlen, const char *name, u32 nlen, char *out, u32 out_len);   /* NAME~1.EXT if taken */
```

Apps that autosave (Notes, Paint) write on every change; the Desk batches the writes to disk once a second and before a handoff.

## Clipboard

```c
void desk_clip_put(u32 format, const u8 *data, u32 len);
i32  desk_clip_get(u32 format, u8 *out, u32 out_len);   /* length, or -1 if empty */
```

Formats: `text` (UTF-8) and `bitmap` (8-bit, with its 768-byte palette first).

## Sound

```c
i32  desk_sound_devices(void);                  /* bits: sb16, opl3, opl2, speaker, mpu401 */
void desk_sound_effect(u32 effect);             /* the Desk's own effects */
u32  desk_midi_open(u32 device);
void desk_midi_send(u32 h, const u8 *bytes, u32 len);
void desk_midi_close(u32 h);
void desk_opl_write(u32 reg, u32 value);        /* the focused app only */
u32  desk_wave_open(u32 rate, u32 bits, u32 channels);   /* Sound Blaster DSP, auto-init DMA */
i32  desk_wave_write(u32 h, const u8 *pcm, u32 len);     /* bytes accepted; the rest waits for the half-buffer interrupt */
void desk_wave_close(u32 h);
```

The sound driver starts from Fizz DOS's `m4/c/sound.c`, extended with an MPU-401 UART output. MIDI on the OPL2 goes through a General MIDI to OPL2 patch bank in the core.

## Toasts

```c
void desk_toast(u32 app, const char *title, u32 tlen, const char *text, u32 len);
```

One at a time, four seconds, queued. No counts, no colours beyond the accent.

## File types

```c
void desk_type_claim(u32 app, const char *ext, u32 len, u32 icon, u32 flags);
```

Called at install time from `APP.INF`, not at run time. Files opens a file with the app that claims its extension, or asks if several do.

## Desktop widgets

```c
u32  desk_widget_new(u32 app, u32 place, i32 w, i32 h);   /* place: bar, desktop */
void desk_widget_invalidate(u32 widget);
```

Widgets get `paint` and `mouse_*` events like windows. The clock and Jax are widgets. Jax is hidden while a dialog is open and frozen when motion is off in Settings.

## Settings storage

```c
i32  desk_setting_get(u32 app, const char *key, u32 klen, char *out, u32 out_len);
void desk_setting_put(u32 app, const char *key, u32 klen, const char *value, u32 vlen);
```

Each app has its own `[section]` in `DESK.INI`, written when the Desk exits or hands off.

## Timers and time

```c
u32  desk_timer(u32 app, u32 ms, u32 repeat);
void desk_timer_stop(u32 timer);
u32  desk_ticks(void);                           /* ms since start */
void desk_now(u32 *date, u32 *time);             /* BCD, from INT 1Ah */
```

## Rules

- Never allocate in an interrupt handler; the SDK never calls an app from one.
- Never touch the hardware. `desk_opl_write` and `desk_palette_lend` are the only ways past the Desk, and both act only for the focused app.
- Use integers in pixel loops. The x87 is set to 53-bit precision at start and must not be changed.
- An app that panics is unloaded; save often with `desk_setting_put` if it matters.
