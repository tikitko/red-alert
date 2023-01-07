# red-alert

VOSK Library `libvosk.a`: https://github.com/tikitko/voskrust/blob/main/README.md

VOSK Models: https://alphacephei.com/vosk/models

Red Alert Main Configuration `config.yaml`:
```yaml
discord_token: "DISCORD_TOKEN"
lang_id: "ru_RU"
listening_text: "красную тревогу..."
vosk_model_path: "vosk-model-small-ru-0.22"
vosk_log_level: -1
```

Red Alert Guilds Voice Configuration `guilds_voice_config.yaml`:
```yaml
auto_track_guilds_ids: []
base:
  target_words:
    - "красная тревога"
    - "код красный"
  self_words:
    - "запретное слово"
    - "ты плохой"
  aliases:
    "алена": 111111111111111111
    "вадим": 222222222222222222
  similarity_threshold: 0.75
  editors: null
specific:
```

(Optional) Red Alert Log Configuration `log_config.yaml`:
```yaml
refresh_rate: 15 seconds
appenders:
  console:
    kind: console
    encoder:
      pattern: "{d(%Y-%m-%d %H:%M:%S)} | {({l}):5.5} | {f}:{L} — {m}{n}"
  info_file:
    kind: file
    path: "info.log"
    encoder:
      pattern: "{d(%Y-%m-%d %H:%M:%S)} | {({l}):5.5} | {f}:{L} — {m}{n}"
loggers:
  red_alert:
    level: info
    appenders:
      - console
      - info_file
```