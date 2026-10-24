# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Maintain the six native encoding UI catalogs (idempotent)."""
import json
from pathlib import Path

rows = [
    ("Choose the encoding used to read this file.", "选择用于读取此文件的编码。", "選擇用於讀取此檔案的編碼。", "이 파일을 읽을 인코딩을 선택하세요.", "このファイルの読み込み形式を選んでください。", "Выберите кодировку для чтения этого файла."),
    ("Text encoding…", "文本编码…", "文字編碼…", "텍스트 인코딩…", "文字エンコーディング…", "Кодировка текста…"),
    ("Open with encoding…", "指定编码打开…", "指定編碼開啟…", "인코딩을 지정하여 열기…", "エンコーディングを指定して開く…", "Открыть в кодировке…"),
    ("Text encoding", "文本编码", "文字編碼", "텍스트 인코딩", "文字エンコーディング", "Кодировка текста"),
    ("Current encoding", "当前编码", "目前編碼", "현재 인코딩", "現在のエンコーディング", "Текущая кодировка"),
    ("Guessed", "推测", "推測", "추정", "推定", "Предположение"),
    ("Write BOM", "写入 BOM", "寫入 BOM", "BOM 기록", "BOM を付加", "Записать BOM"),
    ("Reopen with encoding", "按此编码重新打开", "以此編碼重新開啟", "이 인코딩으로 다시 열기", "この形式で開き直す", "Перечитать в кодировке"),
    ("Use for saving", "设为保存编码", "設為儲存編碼", "저장 인코딩으로 설정", "保存形式に設定", "Выбрать для сохранения"),
    ("Choose encoding…", "选择编码…", "選擇編碼…", "인코딩 선택…", "エンコーディングを選択…", "Выбрать кодировку…"),
    ("Opened", "已打开", "已開啟", "열림", "開きました", "Открыто"),
    ("Saved to disk", "已保存到磁盘", "已儲存至磁碟", "디스크에 저장됨", "ディスクに保存しました", "Сохранено на диск"),
    ("Save encoding changed · save to apply", "保存编码已更改 · 保存后生效", "儲存編碼已變更 · 儲存後生效", "저장 인코딩 변경됨 · 저장 시 적용", "保存形式を変更 · 保存時に適用", "Кодировка изменена · сохраните файл"),
    ("Reopen reads the file again. Save encoding converts your current text on the next save.", "重新打开会再次读取磁盘文件。保存编码将在下次保存时转换当前文本。", "重新開啟會再次讀取磁碟檔案。儲存編碼將在下次儲存時轉換目前文字。", "다시 열기는 디스크 파일을 읽습니다. 저장 인코딩은 다음 저장 시 현재 텍스트를 변환합니다.", "開き直すとディスクから読み込みます。保存形式は次の保存時に現在のテキストに適用されます。", "Перечитывание загружает файл с диска. Кодировка сохранения применяется к текущему тексту при следующем сохранении."),
    ("Legacy encoding detection is a guess. Check the text and reopen manually if needed.", "传统编码识别仅为推测。请检查文字，必要时手动指定编码重新打开。", "傳統編碼識別僅為推測。請檢查文字，必要時手動指定編碼重新開啟。", "기존 인코딩 감지는 추정입니다. 텍스트를 확인하고 필요하면 인코딩을 지정하여 다시 여세요.", "従来のエンコーディング判定は推定です。文字を確認し、必要なら形式を指定して開き直してください。", "Определение старых кодировок приблизительно. Проверьте текст и при необходимости перечитайте файл вручную."),
    ("Reopen discards edits only after the unsaved changes dialog. Conversion never replaces unsupported characters.", "重新打开前会提示处理未保存的修改。无法表示的字符不会被替换。", "重新開啟前會提示處理未儲存的修改。無法表示的字元不會被替換。", "다시 열기 전 저장하지 않은 변경을 확인합니다. 표현할 수 없는 문자는 대체하지 않습니다.", "未保存の変更は開き直す前に確認します。表現できない文字は置き換えません。", "Перед перечитыванием появится запрос о несохранённых изменениях. Неподдерживаемые символы не заменяются."),
    ("The bytes are not valid in the selected encoding. Choose another encoding to reopen.", "文件字节不符合所选编码。请选择其他编码重新打开。", "檔案位元組不符合所選編碼。請選擇其他編碼重新開啟。", "파일 바이트가 선택한 인코딩에 맞지 않습니다. 다른 인코딩으로 다시 여세요.", "ファイルは選択したエンコーディングで読み込めません。別の形式を選んでください。", "Байты не соответствуют выбранной кодировке. Выберите другую для перечитывания."),
    ("The decoded file contains binary control characters. It cannot be opened as text.", "解码后的文件含二进制控制字符，无法作为文本打开。", "解碼後的檔案含二進位控制字元，無法作為文字開啟。", "디코딩된 파일에 이진 제어 문자가 있어 텍스트로 열 수 없습니다.", "復号したファイルにバイナリ制御文字が含まれるため、テキストとして開けません。", "Декодированный файл содержит двоичные управляющие символы и не может быть открыт как текст."),
    ("This encoding cannot preserve every character. Choose UTF-8 or UTF-16; your edits have been kept.", "此编码无法无损保存所有字符。请选择 UTF-8 或 UTF-16，您的修改已保留。", "此編碼無法無損儲存所有字元。請選擇 UTF-8 或 UTF-16，您的修改已保留。", "이 인코딩은 모든 문자를 보존할 수 없습니다. UTF-8 또는 UTF-16을 선택하세요. 편집 내용은 유지됩니다.", "この形式ではすべての文字を保持できません。UTF-8 または UTF-16 を選んでください。編集内容は保持されています。", "Кодировка не сохраняет все символы без потерь. Выберите UTF-8 или UTF-16. Изменения сохранены в редакторе."),
    ("The selected encoding conflicts with the file BOM. Choose the matching Unicode encoding.", "所选编码与文件 BOM 冲突。请选择相符的 Unicode 编码。", "所選編碼與檔案 BOM 衝突。請選擇相符的 Unicode 編碼。", "선택한 인코딩이 파일 BOM과 충돌합니다. 일치하는 Unicode 인코딩을 선택하세요.", "選択した形式がファイルの BOM と一致しません。対応する Unicode 形式を選んでください。", "Кодировка не соответствует BOM файла. Выберите соответствующую кодировку Unicode."),
    ("UTF-32 is not supported. Convert the file to UTF-8 or UTF-16 first.", "暂不支持 UTF-32。请先将文件转换为 UTF-8 或 UTF-16。", "暫不支援 UTF-32。請先將檔案轉換為 UTF-8 或 UTF-16。", "UTF-32는 지원하지 않습니다. 먼저 UTF-8 또는 UTF-16으로 변환하세요.", "UTF-32 は未対応です。先に UTF-8 または UTF-16 に変換してください。", "UTF-32 не поддерживается. Сначала преобразуйте файл в UTF-8 или UTF-16."),
]
path = Path(__file__).resolve().parents[1] / "locales/ui.json"
catalogs = json.loads(path.read_text(encoding="utf-8"))
for row in rows:
    for code, text in zip(["en", "zh-CN", "zh-TW", "ko", "ja", "ru"], row):
        catalogs[code][row[0]] = text
path.write_text(json.dumps(catalogs, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
