# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Idempotent native search UI catalog update for all six supported locales."""
import json
from pathlib import Path

rows = [
("Global search · Ctrl+Shift+F","全局搜索 · Ctrl+Shift+F","全域搜尋 · Ctrl+Shift+F","전체 검색 · Ctrl+Shift+F","全体検索 · Ctrl+Shift+F","Глобальный поиск · Ctrl+Shift+F"),
("Quick Switcher · Ctrl+P","快速切换 · Ctrl+P","快速切換 · Ctrl+P","빠른 전환 · Ctrl+P","クイック切替 · Ctrl+P","Быстрый переход · Ctrl+P"),
("Filename, path, heading or alias…","文件名、路径、标题或别名…","檔名、路徑、標題或別名…","파일명, 경로, 제목 또는 별칭…","ファイル名、パス、見出し、別名…","Имя, путь, заголовок или псевдоним…"),
("↑ ↓ select · Enter open · Shift+Enter open beside · Esc close","↑ ↓ 选择 · Enter 打开 · Shift+Enter 分屏 · Esc 关闭","↑ ↓ 選取 · Enter 開啟 · Shift+Enter 分割 · Esc 關閉","↑ ↓ 선택 · Enter 열기 · Shift+Enter 나란히 · Esc 닫기","↑ ↓ 選択 · Enter 開く · Shift+Enter 横に開く · Esc 閉じる","↑ ↓ выбор · Enter открыть · Shift+Enter рядом · Esc закрыть"),
("Create Markdown document:","新建 Markdown 文档：","新增 Markdown 文件：","Markdown 문서 만들기:","Markdown 文書を作成:","Создать документ Markdown:"),
("Find and replace","查找与替换","尋找與取代","찾기 및 바꾸기","検索と置換","Поиск и замена"),
("Current document","当前文档","目前文件","현재 문서","現在の文書","Текущий документ"),
("Workspace","工作区","工作區","작업 영역","ワークスペース","Рабочая область"),
("Snapshots","历史快照","歷史快照","스냅샷","スナップショット","Снимки"),
("Use current folder","使用当前目录","使用目前目錄","현재 폴더 사용","現在のフォルダーを使用","Текущая папка"),
("Find…","查找…","尋找…","찾기…","検索…","Найти…"),
("Match case","区分大小写","區分大小寫","대소문자 구분","大文字と小文字を区別","Учитывать регистр"),
("Whole word","全词匹配","全字匹配","단어 단위","単語単位","Слово целиком"),
("Regular expression","正则表达式","正規表示式","정규 표현식","正規表現","Регулярное выражение"),
("Replace with","替换为","取代為","바꿀 내용","置換後","Заменить на"),
("Include glob","包含路径 Glob","包含路徑 Glob","포함 경로 Glob","含むパス Glob","Включить Glob"),
("Exclude glob","排除路径 Glob","排除路徑 Glob","제외 경로 Glob","除外パス Glob","Исключить Glob"),
("Separate globs with ; · **/*.md · notes/**","用 ; 分隔 · **/*.md · notes/**","以 ; 分隔 · **/*.md · notes/**","; 로 구분 · **/*.md · notes/**","; で区切る · **/*.md · notes/**","Разделитель ; · **/*.md · notes/**"),
("Preview changes","预览变更","預覽變更","변경 미리 보기","変更をプレビュー","Предпросмотр изменений"),
("Cancel search","取消搜索","取消搜尋","검색 취소","検索を中止","Отменить поиск"),
("Search error:","搜索错误：","搜尋錯誤：","검색 오류:","検索エラー:","Ошибка поиска:"),
("Review the diff before applying. Workspace replacement saves selected dirty buffers.","请检查差异后应用。工作区替换会保存所选文档的未保存修改。","請檢查差異後套用。工作區取代會儲存所選文件的未儲存修改。","차이를 확인하세요. 작업 영역 바꾸기는 선택한 문서의 미저장 편집도 저장합니다.","差分を確認してください。ワークスペース置換は選択文書の未保存の編集も保存します。","Проверьте различия. Замена в рабочей области сохраняет и несохранённые правки выбранных документов."),
("Apply selected","应用所选替换","套用所選取代","선택 항목 적용","選択を適用","Применить выбранное"),
("Select all","全选","全選","모두 선택","すべて選択","Выбрать всё"),
("Select none","取消全选","取消全選","선택 해제","選択を解除","Снять выбор"),
("Replace this file","替换此文件","取代此檔案","이 파일 바꾸기","このファイルを置換","Заменить в файле"),
("Replace this match","替换此匹配","取代此符合項目","이 항목 바꾸기","この一致を置換","Заменить совпадение"),
("Line","行","行","줄","行","Строка"),
("Only the first 200 matches are shown; use filters to narrow the preview.","仅显示前 200 项；请用过滤器缩小预览范围。","僅顯示前 200 項；請用篩選器縮小預覽範圍。","처음 200개만 표시합니다. 필터로 미리 보기 범위를 줄이세요.","最初の200件を表示します。フィルターで絞り込んでください。","Показаны первые 200 совпадений; сузьте предпросмотр фильтрами."),
("matches","项匹配","項符合","개 일치","件の一致","совпадений"),
("Show more files","显示更多文件","顯示更多檔案","파일 더 보기","さらに表示","Показать ещё файлы"),
("Replacement snapshots","替换历史快照","取代歷史快照","바꾸기 스냅샷","置換スナップショット","Снимки замен"),
("Rollback checks current disk and open buffers before restoring original bytes.","回滚前会检查磁盘和已打开文档，然后恢复原始字节。","回復前會檢查磁碟和已開啟文件，再還原原始位元組。","롤백은 디스크와 열린 문서를 확인한 후 원본 바이트를 복원합니다.","復元前にディスクと開いている文書を検証し、元のバイト列に戻します。","Перед восстановлением исходных байтов проверяются диск и открытые документы."),
("No replacement snapshots.","暂无替换快照。","尚無取代快照。","바꾸기 스냅샷이 없습니다.","置換スナップショットはありません。","Снимков замен нет."),
("Rollback","回滚","回復","롤백","復元","Восстановить"),
("Saved snapshot:","已保存快照：","已儲存快照：","저장된 스냅샷:","保存済みスナップショット:","Снимок сохранён:"),
("Snapshot restored.","快照已恢复。","快照已還原。","스냅샷이 복원되었습니다.","スナップショットを復元しました。","Снимок восстановлен."),
("Working in background…","正在后台处理…","正在背景處理…","백그라운드에서 처리 중…","バックグラウンドで処理中…","Фоновая обработка…"),
("Unsaved second document","第二文档尚未保存","第二文件尚未儲存","저장하지 않은 두 번째 문서","2つ目の文書が未保存","Второй документ не сохранён"),
("Save your edits before closing?","关闭前保存修改？","關閉前儲存修改？","닫기 전에 편집 내용을 저장할까요?","閉じる前に保存しますか？","Сохранить правки перед закрытием?"),
("Save or close the second document before quitting.","请先保存或关闭第二文档，再退出。","請先儲存或關閉第二文件，再結束。","종료하기 전에 두 번째 문서를 저장하거나 닫으세요.","終了する前に2つ目の文書を保存するか閉じてください。","Перед выходом сохраните или закройте второй документ."),
("Save or close the second document before opening another.","打开另一文档前，请先保存或关闭第二文档。","開啟其他文件前，請先儲存或關閉第二文件。","다른 문서를 열기 전에 두 번째 문서를 저장하거나 닫으세요.","別の文書を開く前に2つ目の文書を保存するか閉じてください。","Перед открытием другого документа сохраните или закройте второй."),
("This document is already open in the main pane.","此文档已在主窗格中打开。","此文件已在主窗格中開啟。","이 문서는 기본 창에 이미 열려 있습니다.","この文書はメインペインで開いています。","Документ уже открыт в основной панели."),
("Stale search result: refresh the search","搜索结果已过期，请刷新搜索","搜尋結果已過期，請重新整理","검색 결과가 오래되었습니다. 새로 고치세요.","検索結果が古くなりました。更新してください。","Результат устарел. Обновите поиск."),
("Stale diff: an open document changed; refresh the preview","预览已过期：已打开文档发生变更，请重新预览","預覽已過期：已開啟文件有所變更，請重新預覽","열린 문서가 변경되었습니다. 미리 보기를 새로 고치세요.","開いている文書が変更されました。差分を更新してください。","Открытый документ изменился. Обновите предпросмотр."),
]
path=Path(__file__).resolve().parents[1]/"locales/ui.json"
catalogs=json.loads(path.read_text(encoding="utf-8"))
for row in rows:
    for code,text in zip(["en","zh-CN","zh-TW","ko","ja","ru"],row): catalogs[code][row[0]]=text
path.write_text(json.dumps(catalogs,ensure_ascii=False,indent=2)+"\n",encoding="utf-8",newline="\n")
