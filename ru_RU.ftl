help-command-prefix-anchor = кринж киллер помощь
help-command-full-header = > **`{$header} {$suffix}`**
help-command-short-header = > **`{$header}`**
help-command-body = ```{$body}```
red-alert-command-prefix-anchor = код красный
red-alert-command-header-suffix = {"{"}ID или упоминание пользователя{"}"}*
red-alert-command-help-description =
    {"*"} - может быть несколько (через пробел).
    Кикает выбранного пользователя из голосового канала если он в нем находится, иначе, кикает исполнителя команды.
red-alert-command-empty-self-success = ВИЖУ ТЫ ЗАБЫЛ УКАЗАТЬ ЦЕЛЬ ДЛЯ КРАСНОГО КОДА, НИЧЕГО... ШМАЛЬНЕМ В ТЕБЯ! (ИСПОЛЬЗУЙ ТЕГИ) ПРИНЯТО К ИСПОЛНЕНИЮ!
red-alert-command-empty-self-not-found = :face_with_monocle: ПОЛЬЗУЙСЯ ТЕГАМИ, И ЛУЧШЕ НЕ ЗАХОДИ В КАНАЛ, А ТО КИКНУ С ТАКИМИ ПРИКОЛАМИ! Пшшшш...
red-alert-command-empty-self-error = СЛОМАЛСЯ ПОКА ПЫТАЛСЯ ТЕБЯ КИКНУТЬ ЧТО НЕПРАВИЛЬНОЕ ИСПОЛЬЗОВАНИЕ, КАК ВСЕГДА КОД ГОВНА! ОТМЕНА! Пшшшш...
red-alert-command-single-target-success = КОД КРАСНЫЙ ПОДТВЕРЖДЕН! АНТИКРИНЖ ОРУЖИЕ ИСПОЛЬЗОВАНО ПРОТИВ {$user-name}!!! 0)00))00
red-alert-command-single-self-success = КОД КРАСНЫЙ ПОДТВЕРЖДЕН! САМОВЫПИЛ ДЕЛО ДОСТОЙНОЕ!!! 0)00))00
red-alert-command-single-not-found-self-success = В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЗНАЧИТ У ТЕБЯ БЕДЫ С БОШКОЙ, КОД КРАСНЫЙ НА ТЕБЯ!
red-alert-command-single-not-found-self-not-found = ДОФИГА УМНЫЙ ВИЖУ? В КАНАЛЕ НЕТ ЧЕЛА ДЛЯ КОДА КРАСНОГО, ЖАЛЬ ТЕБЯ В КАНАЛЕ НЕТУ, ТАК БЫ ТЕБЯ ШМАЛЬНУЛ КОДОМ КРАСНЫМ! ОТМЕНА! Пшшшш...
red-alert-command-single-not-found-self-error = ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ТО ЧТО ТЫ ПЫТАЛСЯ КИКНУТЬ ТОГО КОГО НЕТ, НО Я СЛОМАЛСЯ! Пшшшш...
red-alert-command-single-not-found-self = СУИЦИД ЭТО ПЛОХО ТАК ЧТО НЕТ))) (У меня просто не получилось)
red-alert-command-single-target-error = АУЧ, МАСЛИНУ ПОЙМАЛ, ОШИБКА В СИСТЕМЕё0))
red-alert-command-single-self-error = АУЧ, МАСЛИНУ ПОЙМАЛ, НЕ СМОГ ОРГАНИЗОВАТЬ ТЕБЕ СУИЦИД0))
red-alert-command-mass-self-success = МАССОВЫЙ КОД КРАСНЫЙ ШТУКА ОПАСНАЯ, ТАК КАК ПО РАЗНЫМ ПРИЧИНАМ Я НИКОГО НЕ КИКНУЛ, КИКНУ ТЕБЯ )В)В)))0
red-alert-command-mass-self-not-found = ЖАЛЬ ТЕБЯ НЕ МОГУ ПРШИТЬ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ! ОТМЕНА Пшшшш...
red-alert-command-mass-self-error = ХОТЕЛ ШМАЛЬНУТЬ В ТЕБЯ ЗА ЛОЖНЫЙ КОД КРАСНЫЙ, НО САМ ОБО****СЯ! Пшшшш...
red-alert-command-mass-success-status = ИСПОЛНЕНО
red-alert-command-mass-not-found-status = НЕ В КАНАЛЕ
red-alert-command-mass-error-status = ОШИБКА (ПРОЧНЫЙ СУ*А)
red-alert-command-mass-record = {$record-number}. {$user-name} СТАТУС: {$deport-status}.
red-alert-command-mass-records-header = ОУ, МАССОВЫЙ КОД КРАСНЫЙ? СТАТУС ВЫКОСА КРИНЖОВИКОВ:
start-listen-red-alert-command-prefix-anchor = слушать код красный
start-listen-red-alert-command-header-suffix = {"{"}ID или упоминание канала{"}"}*
start-listen-red-alert-command-help-description = Начать слушать выбранный голосовой канал на запрещенные и направленные фразы.
start-listen-red-alert-command-success = ОТСЛЕЖИВАЮ КОД КРАСНЫЙ В КАНАЛЕ {$channel-name}...
start-listen-red-alert-command-connect-error = ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {$channel-name}. НЕ ПОЛУЧАЕТСЯ ВОЙТИ В КАНАЛ...
start-listen-red-alert-command-lib-error = ОШИБКА СЛЕЖКИ ЗА КАНАЛОМ {$channel-name}. ЗВУКОВАЯ БИБЛИОТЕКА ОТСУТСТВУЕТ...
start-listen-red-alert-command-missed-channel = ЧТО ОТСЛЕЖИВАТЬ НАРКОМАН?
stop-listen-red-alert-command-prefix-anchor = прекратить слушать код красный
stop-listen-red-alert-command-help-description = Прекратить слушать голосовой канал в котором находится КРИНЖ КИЛЛЕР на запрещенные и направленные фразы.
stop-listen-red-alert-command-success = ПРЕКРАЩАЮ ОТСЛЕЖИВАНИЕ КАНАЛА!
stop-listen-red-alert-command-disconnect-error = ПРОИЗОШЛА ОШИБКА! НЕ ПОЛУЧАЕТСЯ ОТКЛЮЧИТЬСЯ...
stop-listen-red-alert-command-lib-error = ЗВУКОВАЯ БИБЛИОТЕКА ОТСУТСТВУЕТ...
stop-listen-red-alert-command-no-channel = НЕ ОТСЛЕЖИВАЮ КАНАЛЫ!
actions-history-red-alert-command-prefix-anchor = код красный история
actions-history-red-alert-command-help-description = Выводит историю всех наказаний которые исполнил КРИНЖ КИЛЛЕР.
actions-history-red-alert-command-list-header = ИСТОРИЯ ВЫКОСА КРИНЖОВИКОВ:
actions-history-red-alert-command-self-kick-status-success = САМОВЫПИЛИЛСЯ
actions-history-red-alert-command-self-kick-status-fail = ПОПЫТАЛСЯ САМОВЫПИЛИТЬСЯ
actions-history-red-alert-command-target-kick-status-success = КИКНУТ
actions-history-red-alert-command-target-kick-status-fail = ПОЧТИ... КИКНУТ
actions-history-red-alert-command-voice-self-record = КРИНЖОВИК {$target-name} {$status} ФРАЗОЙ "{$reason}".
actions-history-red-alert-command-voice-target-record = КРИНЖОВИК {$target-name} {$status} ГОЛОСОМ МИРОТВОРЦA {$autor-name} ПРИ ПОМОЩИ ФРАЗЫ "{$reason}".
actions-history-red-alert-command-text-self-record = КРИНЖОВИК {$target-name} {$status} КОМАНДОЙ
actions-history-red-alert-command-text-target-record = КРИНЖОВИК {$target-name} {status} КОМАНДОЙ МИРОТВОРЦA {$autor-name}
actions-history-red-alert-command-record = {$record-number}. [ВРЕМЯ: {$time}] {$record}.
actions-history-red-alert-command-empty-list = ПОКА ЕЩЕ НИКОГО НЕ УШАТАЛ НА ЭТОМ СЕРВЕР)!1!))