---
name: IRE Design System
colors:
  surface: '#0e0e11'
  surface-dim: '#0e0e11'
  surface-bright: '#2a2b35'
  surface-container-lowest: '#000000'
  surface-container-low: '#131317'
  surface-container: '#19191e'
  surface-container-high: '#1e1f26'
  surface-container-highest: '#24252d'
  on-surface: '#e6e4ef'
  on-surface-variant: '#abaab4'
  inverse-surface: '#fbf8fc'
  inverse-on-surface: '#555458'
  outline: '#75757e'
  outline-variant: '#474750'
  surface-tint: '#c6c6c9'
  primary: '#c6c6c9'
  on-primary: '#3f4043'
  primary-container: '#454749'
  on-primary-container: '#d0d0d3'
  inverse-primary: '#5e5f62'
  secondary: '#9d9da6'
  on-secondary: '#1f2027'
  secondary-container: '#3a3b43'
  on-secondary-container: '#bfbec8'
  tertiary: '#f9f9fd'
  on-tertiary: '#5e5f62'
  tertiary-container: '#ebebef'
  on-tertiary-container: '#55575a'
  error: '#ec7c8a'
  on-error: '#490013'
  error-container: '#7f2737'
  on-error-container: '#ff97a3'
  primary-fixed: '#e2e2e5'
  primary-fixed-dim: '#d4d4d7'
  on-primary-fixed: '#3e3f42'
  on-primary-fixed-variant: '#5a5b5e'
  secondary-fixed: '#e3e1ec'
  secondary-fixed-dim: '#d4d3dd'
  on-secondary-fixed: '#3e3f47'
  on-secondary-fixed-variant: '#5a5b63'
  tertiary-fixed: '#f3f3f7'
  tertiary-fixed-dim: '#e5e5e9'
  on-tertiary-fixed: '#484a4d'
  on-tertiary-fixed-variant: '#65666a'
  primary-dim: '#b8b8bb'
  secondary-dim: '#9d9da6'
  tertiary-dim: '#ebebef'
  error-dim: '#b95463'
  background: '#0e0e11'
  on-background: '#e6e4ef'
  surface-variant: '#24252d'
typography:
  headline-lg:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: '600'
    lineHeight: '1.2'
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '600'
    lineHeight: '1.3'
  body-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: '1.6'
  code:
    fontFamily: JetBrains Mono
    fontSize: 14px
    fontWeight: '400'
    lineHeight: '1.5'
  label-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '500'
    lineHeight: '1.0'
    letterSpacing: 0.05em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
---

# IRE Design System — Dark Mode

Dark counterpart of the IRE design system. Same principles, type, spacing, and shape — only surface tones invert. See the Light variant for the full reference.

## Surfaces
- bg #0A0A0A (canvas, near-black, OLED-friendly)
- bg-subtle #111113 (sidebars)
- bg-muted #1A1A1D (code, hover)
- border #27272A, border-strong #3F3F46

## Foreground
- fg #F4F4F5 (off-white, easier on the eyes than pure white)
- fg-muted #A1A1AA, fg-subtle #71717A
- accent #E4E4E7 (light graphite for buttons; inverse of light)
- accent-fg #0A0A0A, link #D4D4D8

## State (slightly lighter than Light variants for contrast on near-black)
- state-error #F87171, state-warn #FBBF24, state-ok #4ADE80, state-info #60A5FA

## Notes
- Backgrounds are flat. Hierarchy comes from 1px borders and small tone steps (bg → bg-subtle → bg-muted), each ≤ 3% lighter than the previous.
- Accent inverts: light graphite on near-black mirrors near-black on white in Light. No chromatic accent in either mode.
- Code blocks use bg-muted with JetBrains Mono. Two-tone syntax (fg for code, fg-muted for comments) — no rainbow.