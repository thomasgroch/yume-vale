# Yume Vale — Pastel Design Language

> A soft, inviting visual language for a multiplayer 3D world where foxes explore together.  
> No combat — shared presence in a cute vale.

---

## Palette

All colours are sRGB. Accessible pairs meet **WCAG 2.1 AA for large text** (contrast ≥ 3:1).

### Surface

| Token | Value | Swatch | Usage |
|---|---|---|---|
| `SURFACE_MENU` | `rgb(255, 230, 240)` → `(1.0, 0.90, 0.94)` | ██ light pink | Menu page background |
| `SURFACE_RECONNECT` | `rgb(64, 64, 77)` → `(0.25, 0.25, 0.3)` | ██ dark neutral | Reconnect button background |

### Text

| Token | Value | Swatch | Contrast vs SURFACE_MENU | Usage |
|---|---|---|---|---|
| `TEXT_TITLE` | `rgb(224, 107, 148)` → `(0.88, 0.42, 0.58)` | ██ rose | ≥ 3.0 ✅ | Game title |
| `TEXT_SUBTLE` | `rgb(110, 128, 148)` → `(0.43, 0.50, 0.58)` | ██ slate blue‑grey | ≥ 3.0 ✅ | Subtitle, hints, secondary labels |
| `Color::WHITE` | `rgb(255, 255, 255)` → `(1.0, 1.0, 1.0)` | ██ white | ≥ 3.0 ✅ | Button label, HUD text |

### Interactive

| Token | Value | Swatch | Usage |
|---|---|---|---|
| `BUTTON_PRIMARY` | `rgb(255, 143, 171)` → `(1.0, 0.56, 0.67)` | ██ pink | Primary play button |
| `BUTTON_PRIMARY_HOVER` | `rgb(255, 122, 153)` → `(1.0, 0.48, 0.60)` | ██ darker pink | Hover / pressed state |

### Status indicators (HUD)

| Token | Value | Swatch | Usage |
|---|---|---|---|
| `STATUS_OK` | `rgb(102, 230, 102)` → `(0.4, 0.9, 0.4)` | ██ green | Connected |
| `STATUS_BUSY` | `rgb(230, 204, 77)` → `(0.9, 0.8, 0.3)` | ██ yellow | Connecting |
| `STATUS_ERR` | `rgb(230, 102, 102)` → `(0.9, 0.4, 0.4)` | ██ red | Disconnected / error |

### Decorative bubbles (menu)

| Token | Value | Swatch | Usage |
|---|---|---|---|
| `BUBBLE_PINK` | `rgba(255, 199, 217, 0.5)` | ██ translucent pink | Floating ornament |
| `BUBBLE_BLUE` | `rgba(199, 230, 255, 0.5)` | ██ translucent blue | Floating ornament |
| `BUBBLE_GREEN` | `rgba(204, 245, 217, 0.5)` | ██ translucent green | Floating ornament |

### Touch overlay

| Token | Value | Swatch | Usage |
|---|---|---|---|
| `OVERLAY_JUMP` | `rgba(255, 255, 255, 0.18)` | ██ very faint white | Jump button circle |
| `OVERLAY_JUMP_TEXT` | `rgba(255, 255, 255, 0.7)` | ██ medium white | Jump label |
| `OVERLAY_RING` | `rgba(255, 255, 255, 0.10)` | ██ faint white | Joystick ring |
| `OVERLAY_KNOB` | `rgba(255, 255, 255, 0.25)` | ██ low white | Joystick knob |

---

## Typography

All UI uses Bevy's **default font** (no custom `.ttf` loaded). Font sizes are **px**:

| Token | Size | Usage |
|---|---|---|
| `FONT_XS` | 11 | Version / debug overlay |
| `FONT_SM` | 14 | Controls hint (bottom‑centred) |
| `FONT_MD` | 16 | Touch button label, status text |
| `FONT_LG` | 20 | Subtitle |
| `FONT_XL` | 28 | Button label |
| `FONT_TITLE` | 80 | Game title |

Every text entity **should** include a `TextShadow::default()` for legibility over busy backgrounds.

---

## Spacing

Named tokens in `theme::SPACE_*` — all values in **px**:

| Token | Value | Typical use |
|---|---|---|
| `SPACE_4` | 4 | Tight padding |
| `SPACE_6` | 6 | HUD button vertical margin |
| `SPACE_8` | 8 | Title/subtitle gap |
| `SPACE_10` | 10 | HUD corner inset |
| `SPACE_11` | 11 | Version text font |
| `SPACE_14` | 14 | Hint text font |
| `SPACE_16` | 16 | Standard padding axis, touch inset |
| `SPACE_20` | 20 | Subtitle font |
| `SPACE_24` | 24 | Page‑level bottom margin |
| `SPACE_28` | 28 | Button label font |
| `SPACE_32` | 32 | Touch inset (jump button right/bottom margin) |
| `SPACE_48` | 48 | Title‑to‑button gap |
| `SPACE_64` | 64 | Button horizontal padding |
| `SPACE_72` | 72 | Jump button diameter |
| `SPACE_80` | 80 | Title font |

---

## Border radius

| Token | Value | Usage |
|---|---|---|
| `RADIUS_PILL` | 999 px | Buttons, touch circles, joystick rings — fully rounded pill shape |
| `BorderRadius::MAX` | — | Decorative floating bubbles (menu) |

---

## Interaction states

| Component | Resting | Hover | Pressed |
|---|---|---|---|
| Primary button (`PlayButton`) | `BUTTON_PRIMARY` | `BUTTON_PRIMARY_HOVER` | `BUTTON_PRIMARY_HOVER` |
| Jump button (touch) | `OVERLAY_JUMP` | (unchanged) | (tracked by `TouchJump`) |
| Reconnect button (HUD) | `SURFACE_RECONNECT` | (unchanged) | (triggers reconnect) |

The helper `theme::button_interaction_color()` maps `Interaction → Color` for primary buttons.

---

## Responsive design

Breakpoints are not currently enforced in code. The UI layout adapts:

| Breakpoint | Behaviour |
|---|---|
| ≥ 1024 px (desktop) | Controls hint visible, touch UI hidden, camera drag+scroll |
| 768–1023 px (tablet) | Touch UI visible if touch detected, else unchanged |
| < 768 px (mobile) | Touch UI always shown (auto-detected), controls hint hidden |

Touch detection happens in `touch::detect_touch` — once any touch is registered, the jump button and joystick become visible.

---

## Accessibility

- **Colour contrast** — all interactive text pairs are verified in tests (`interactive_colors_have_accessible_contrast`). Minimum target: **3:1** (WCAG AA large text).
- **Interaction targets** — touch buttons are ≥ 72 px diameter (jump button) or ≥ 44 px (joystick knob), exceeding the 44 px WCAG minimum.
- **Keyboard navigation** — WASD/arrow keys move; Shift runs; Space jumps; Q/E rotates camera. No tab‑stop focus indicators yet (future work, task 20).

---

## Source of truth

All tokens live in `crates/game_client/src/ui/theme.rs`.  
Widget builders live in `crates/game_client/src/ui/widgets.rs`.  
No raw colour, spacing, or radius literal should appear in any UI screen file.
