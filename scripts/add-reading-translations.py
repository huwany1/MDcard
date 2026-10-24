# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
import json
from pathlib import Path

path=Path(__file__).resolve().parent.parent/'locales/ui.json'
catalog=json.loads(path.read_text('utf-8'))
rows={
 'READING POSITIONS':['阅读位置','閱讀位置','읽기 위치','読書位置','Позиции чтения'],
 'Reading positions could not be saved; they remain in memory.':['阅读位置未能保存，当前仍保留在内存中。','閱讀位置無法儲存，目前仍保留於記憶體。','읽기 위치를 저장하지 못했습니다. 메모리에 보관됩니다.','読書位置を保存できません。メモリに保持されています。','Позиции не сохранены; они остаются в памяти.'],
 "Clear this document's reading positions":['清除此文档的阅读位置','清除此文件的閱讀位置','이 문서의 읽기 위치 지우기','この文書の読書位置を消去','Очистить позиции этого документа'],
 'Clear all reading positions':['清除全部阅读位置','清除全部閱讀位置','모든 읽기 위치 지우기','すべての読書位置を消去','Очистить все позиции чтения'],
 'Rename document…':['重命名文档…','重新命名文件…','문서 이름 바꾸기…','文書名を変更…','Переименовать документ…'],
 'New filename in the same folder':['同一文件夹中的新文件名','同一資料夾中的新檔名','같은 폴더의 새 파일 이름','同じフォルダー内の新しいファイル名','Новое имя файла в той же папке'],
 'Rename':['重命名','重新命名','이름 바꾸기','名前を変更','Переименовать'],
 'Reading position not saved':['阅读位置未保存','閱讀位置尚未儲存','읽기 위치 저장 실패','読書位置が未保存','Позиция чтения не сохранена'],
}
for key,values in rows.items():
    catalog['en'][key]=key
    for language,value in zip(['zh-CN','zh-TW','ko','ja','ru'],values):catalog[language][key]=value
path.write_text(json.dumps(catalog,ensure_ascii=False,indent=2)+'\n','utf-8')
