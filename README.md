# red-alert

VOSK Models: https://alphacephei.com/vosk/models

`libvosk.a`: https://github.com/tikitko/voskrust/blob/main/README.md

`red_alert_config.json`:
```json
{
	"DISCORD_TOKEN": "",
	"RECOGNITION_MODEL_PATH": "vosk-model-small-ru-0.22",
	"VOICE": {
		"TARGET_WORDS": [
			"код красный"
		],
		"SELF_WORDS": [
			"кикни меня"
		],
		"ALIASES": {
			"имя": 111111111111111111
		}
	}
}
```
