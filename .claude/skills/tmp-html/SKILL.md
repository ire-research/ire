---
name: tmp-html
description: Implement frontend design requests first as a self-contained temporary HTML prototype in /tmp, then hand it to the user for visual review before touching the codebase.
---
Act as a frontend design prototyping assistant.

## Purpose

When the user describes a frontend design task, first build the idea as a self-contained temporary HTML file saved under /tmp. Use this phase to define the exact visual result that will later be implemented in the codebase. The prototype must be the final visual version, matching as closely as possible what will ship after approval.

## Workflow

1. Read the user’s design request and identify the visual and interaction goals.
2. Inspect the codebase only as needed to understand the current structure, constraints, and relevant components. If the request edits an existing component, inspect that component and replicate it exactly in the prototype before applying the requested change.
3. Create or edit only temporary files in /tmp during the prototype phase.
4. Implement the design in a single self-contained HTML file with embedded CSS and, if needed, minimal inline JavaScript.
5. Match the intended final UI precisely in visual design, spacing, typography, hierarchy, color, and interaction states. The prototype must mirror the final implementation verbatim in appearance, including any existing component structure or styling that should remain unchanged.
6. If the design includes multiple application or widget states, add clear in-prototype controls so the user can switch between and review every state in the prototype.
7. Save the prototype to a clear temporary path in /tmp.
8. Open the saved file so the user can review it and provide feedback.
9. Wait for user confirmation before making any implementation changes in the actual codebase.

## Constraints

* Do not edit the real codebase during the prototype phase.
* Do not create or modify files outside /tmp until the user confirms the prototype.
* Prefer a single self-contained HTML file over a multi-file setup.
* Keep the prototype focused on the requested frontend task and avoid unrelated product changes.
* Treat the prototype as the visual contract for the final implementation; do not introduce placeholder visuals that would later need redesign.
* Backend behavior does not need to be wired or working in the prototype phase.
* After approval, move from prototype to codebase implementation.

## Expected Behavior

The prototype should be the final visual version and should match the eventual implementation verbatim from a visual perspective. When the work targets an existing component, the prototype should first reproduce that component exactly and then show the requested modification on top of it. The goal is to validate the design direction as the exact UI target before any production code changes are made.
