---
name: IRE Design System
colors:
  surface: '#fbf8fc'
  surface-dim: '#dad9e5'
  surface-bright: '#fbf8fc'
  surface-container-lowest: '#ffffff'
  surface-container-low: '#f5f2f8'
  surface-container: '#efedf4'
  surface-container-high: '#e9e7f0'
  surface-container-highest: '#e3e1ec'
  on-surface: '#31323a'
  on-surface-variant: '#5e5e67'
  inverse-surface: '#0e0e11'
  inverse-on-surface: '#9e9ca0'
  outline: '#7a7a83'
  outline-variant: '#b2b1bb'
  surface-tint: '#5f5e61'
  primary: '#5f5e61'
  on-primary: '#faf7fb'
  primary-container: '#e4e1e5'
  on-primary-container: '#525155'
  inverse-primary: '#fefbff'
  secondary: '#5e5e67'
  on-secondary: '#faf8ff'
  secondary-container: '#e3e1ec'
  on-secondary-container: '#51515a'
  tertiary: '#5e5e67'
  on-tertiary: '#faf8ff'
  tertiary-container: '#eeedf7'
  on-tertiary-container: '#575860'
  error: '#9e3f4e'
  on-error: '#fff7f7'
  error-container: '#ff8b9a'
  on-error-container: '#782232'
  primary-fixed: '#e4e1e5'
  primary-fixed-dim: '#d6d3d7'
  on-primary-fixed: '#3f3f42'
  on-primary-fixed-variant: '#5c5b5e'
  secondary-fixed: '#e3e1ec'
  secondary-fixed-dim: '#d5d3de'
  on-secondary-fixed: '#3e3f47'
  on-secondary-fixed-variant: '#5b5b64'
  tertiary-fixed: '#eeedf7'
  tertiary-fixed-dim: '#dfdfe8'
  on-tertiary-fixed: '#45464d'
  on-tertiary-fixed-variant: '#61626a'
  primary-dim: '#525255'
  secondary-dim: '#52525b'
  tertiary-dim: '#52535a'
  error-dim: '#4f0116'
  background: '#fbf8fc'
  on-background: '#31323a'
  surface-variant: '#e3e1ec'
typography:
  headline-lg:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '600'
    lineHeight: 32px
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Inter
    fontSize: 18px
    fontWeight: '600'
    lineHeight: 24px
    letterSpacing: -0.01em
  body-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 20px
  body-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 16px
  label-sm:
    fontFamily: Inter
    fontSize: 11px
    fontWeight: '600'
    lineHeight: 14px
    letterSpacing: 0.05em
  code:
    fontFamily: JetBrains Mono
    fontSize: 13px
    fontWeight: '400'
    lineHeight: '1.5'
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  base: 4px
  '1': 4px
  '2': 8px
  '3': 12px
  '4': 16px
  '5': 20px
  '6': 24px
  '8': 32px
  pane-padding: 16px
  section-gap: 24px
---

# IRE Design System

## Product
IRE is a local-first desktop app for ML researchers. It wraps Claude-Code in a 5-pane workspace: focus, resources, experiments, central chat, notes, ideas. Users are academics — they live in this app for hours. The UI must disappear so the work shows through.

## Principles
1. Calm over loud. Low saturation. No accent colors that pull the eye.
2. Information density. Tight spacing, smaller type, more visible at once.
3. Text-first, no decoration. No illustrations, no hero images, no gradients, no shadows beyond a 1px hairline border.
4. Monochrome by default, color by signal only (error/warn/ok/info).
5. Borders, not fills. 1px hairlines for separation.
6. Honor the OS. Native scrollbars and chrome on Tauri.

## Color (Light)
- bg #FFFFFF, bg-subtle #FAFAFA, bg-muted #F4F4F5
- border #E4E4E7, border-strong #D4D4D8
- fg #18181B, fg-muted #52525B, fg-subtle #A1A1AA
- accent #27272A, accent-fg #FAFAFA, link #3F3F46
- state-error #B91C1C, state-warn #A16207, state-ok #15803D, state-info #1E40AF

## Color (Dark, see paired system)
- bg #0A0A0A, bg-subtle #111113, bg-muted #1A1A1D
- fg #F4F4F5, accent #E4E4E7

## Type
Inter everywhere; JetBrains Mono for code. Bold = 600 max. Tight tracking on display sizes.

## Shape & Spacing
4px radius. 4px spacing base, scale 0–16. Pane padding 16, section gap 24.

## Components
Button 28–32px / 1px border / no shadow. Input 28px / 1px border. Pane = 24px header strip + bordered region. List item 24px tall, 2px left bar when active. Chat: assistant full-width text on canvas, user right-aligned with bg-muted. Tags 18px pills.

## Layout
5-pane desktop grid ≥ 1280px. Left rail 240, right rail 320, center fluid. Right collapses < 1024, left collapses < 800.

## Motion
120ms hover, 180ms panel. No springs. Respect prefers-reduced-motion.