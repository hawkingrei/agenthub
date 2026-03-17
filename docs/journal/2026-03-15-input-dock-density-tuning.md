# Input Dock Density Tuning

## Summary

- compressed the agent input dock so mobile screens reserve more height for conversation content
- reduced dock chrome weight without changing keyboard, history, or interrupt behavior
- aligned the dock controls with the tighter mobile header and ACP panel density

## Details

- reduced `input.docked` padding, gap, radius, and shadow weight
- reduced `Send`, `History`, and `Interrupt` control heights and horizontal padding
- lowered textarea minimum height on both default and narrow-screen layouts
- tightened history menu spacing and item density
- reduced jump-to-bottom button size to keep the footer visually lighter

## Validation

- baseline inspection used Chrome DevTools MCP against `https://agenthub.hawkingrei.com/` at `390x844` to confirm the current dock still takes a large vertical slice on mobile
- local regression should verify the dock remains usable for typing, history recall, interrupt, and jump-to-bottom interactions after density tuning
