# Trinity Clipboard

Text clipboard history module for Trinity.

## Capabilities

- Polls the system clipboard on a background thread.
- Stores non-empty text entries with duplicate promotion.
- Persists history in `~/.trinity/clipboard_history.json`.
- Provides an egui picker with Up/Down selection, Left/Right pagination, Enter paste, and Escape close.

## Notes

History is text-only. The picker sets the selected text as the system clipboard content before simulating paste into the foreground application.
