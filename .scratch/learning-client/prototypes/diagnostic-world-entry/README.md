# Diagnostic World-entry experience prototype

Throwaway browser mock for deciding how the Learning Client should explain world entry, movement, and correction before the real Bevy shell exists.

This is deliberately **not** evidence that Bevy builds or runs. It uses scripted engine-free events and project-created browser primitives so the interaction model can be judged cheaply.

## Run

From the repository root:

```sh
python3 -m http.server 4173 --directory .scratch/learning-client/prototypes/diagnostic-world-entry
```

Open <http://localhost:4173/>.

The selected design is the viewport-first cockpit. Use camera-relative `WASD` after world entry to move the placeholder, right-mouse drag the scene to orbit, use the wheel to zoom, and use **Connect & Enter** or the other scenario controls to step through the scripted flow.

## Throwaway boundary

Do not promote this HTML/CSS/JavaScript into the Rust workspace. Once the experience decision is captured in the issue, delete this directory. The selected behavior must be rewritten against Bevy's built-in UI and project-owned 3D primitives.
