# Lightfriend important-things walkthrough

A 36-second, 16:9 motion walkthrough of Lightfriend's core flow:

1. Selected connected messages arrive in Lightfriend.
2. Routine noise waits while time-sensitive information can surface by SMS.
3. The user asks Lightfriend for current messages and tracked events on demand.

The copy reflects the current product implementation. In particular, the video does not claim a dedicated Google Calendar connection; "tracked events" are obligations extracted from connected messages and queried through Lightfriend's ontology tools.

## Render

Requirements: `agent-browser` with Chromium and `ffmpeg`.

```bash
chmod +x render.sh
./render.sh
```

The script warms up Chromium's recorder before starting the paused CSS
timeline, then trims that pre-roll automatically. It creates:

- `lightfriend-important-things.webm` — raw browser capture
- `lightfriend-important-things.mp4` — H.264 delivery file

To preview without rendering, open `index.html` and call `window.startFilm()` in the browser console. Add `?autoplay` to start automatically.
