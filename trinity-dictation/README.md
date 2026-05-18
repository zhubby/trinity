# Trinity Dictation

Voice dictation input module for Trinity.

## Capabilities

- Records the default microphone while the dictation hotkey is held.
- Encodes captured audio as WAV.
- Sends audio to ElevenLabs Speech to Text.
- Types recognized text into the currently focused input with direct text injection.

## Configuration

Settings are read from `~/.trinity/config.json` through `trinity-util`:

```json
{
  "hotkey": {
    "record_dictation": "Command+Shift+Space"
  },
  "dictation": {
    "provider": "elevenlabs",
    "api_key": "YOUR_ELEVENLABS_API_KEY",
    "model_id": "scribe_v2",
    "language_code": null
  }
}
```

`language_code` can be left `null` for automatic language detection.
