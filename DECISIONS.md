# LightningFiler 仮決定レポート

本ドキュメントは、設計書に曖昧さがあった部分について、実装者（Claude）が下した仮決定を記録する。
オリジナル設計者（wai55555）がレビュー後、変更が必要な箇所は修正可能。

---

## 1. 技術選定の仮決定

### 1.1 KVSキャッシュ: RocksDB を採用

**設計書の記述**: RocksDB優先、Windowsビルド困難ならSled代替

**仮決定**: **RocksDB を採用**
- 理由: パフォーマンスが最優先要件
- Windows向けビルドは `vcpkg` + `LLVM_HOME` 設定で対応
- fallback不要と判断（ビルドスクリプトで環境整備）

```toml
# Cargo.toml
rocksdb = { version = "0.22", default-features = false, features = ["lz4"] }
```

**変更が必要な場合**: Sledに切り替える場合はDBインターフェースを trait で抽象化済みのため、実装差し替えで対応可能

---

### 1.2 スクリプト言語: Lua (mlua) を採用

**設計書の記述**: Lua または Rhai を検討

**仮決定**: **Lua (mlua with LuaJIT)** を採用
- 理由:
  - ゲーム業界での実績が豊富
  - 既存ユーザーの学習コスト低
  - mlua クレートの成熟度が高い
  - LuaJIT による高速実行

```toml
mlua = { version = "0.9", features = ["luajit", "serialize"] }
```

**変更が必要な場合**: Rhai は Rust-native で安全性が高いが、Lua ほど普及していないため Lua を選択

---

### 1.3 テキストレンダリング: cosmic-text を採用

**設計書の記述**: `cosmic-text` または `DirectWrite` を検討

**仮決定**: **cosmic-text** を採用
- 理由:
  - クロスプラットフォーム対応（将来のLinux移植可能性）
  - Unicode正規化、複雑なスクリプト（アラビア語等）対応
  - egui との親和性が良好
  - DirectWrite は Windows 依存

```toml
cosmic-text = "0.11"
```

**フォントフォールバック**: システムフォント + Noto Sans CJK + Noto Color Emoji を自動ロード

---

### 1.4 国際化 (i18n): Fluent を採用

**設計書の記述**: Fluent または JSON

**仮決定**: **Fluent** を採用
- 理由:
  - Mozilla 開発、業界標準
  - 複数形・性別・文脈対応
  - 型安全なメッセージ参照

```toml
fluent = "0.16"
fluent-langneg = "0.13"
```

**ファイル配置**:
```
resources/
  locales/
    ja/
      main.ftl
      commands.ftl
    en/
      main.ftl
      commands.ftl
```

---

### 1.5 アーカイブライブラリ

**設計書の記述**: 具体的な選定なし

**仮決定**:
| 形式 | ライブラリ | 備考 |
|------|----------|------|
| ZIP | `zip` | デフォルト、エンコーディング処理カスタム |
| RAR | `unrar` | ライセンス注意（GPL互換要確認） |
| 7z | `sevenz-rust` | Pure Rust実装 |
| LZH | Susie Bridge経由 | レガシー対応 |
| TAR/GZ | `tar` + `flate2` | 標準的 |

```toml
zip = "0.6"
sevenz-rust = "0.5"
tar = "0.4"
flate2 = "1.0"
```

**unrar について**: GPL制約があるため、RAR対応は Susie Bridge (axrar.spi) をデフォルトとし、ネイティブ対応はオプション機能として実装

---

### 1.6 egui バージョン

**設計書の記述**: egui 0.26, egui-wgpu 0.26

**仮決定**: **最新安定版** に更新
```toml
egui = "0.29"
egui-wgpu = "0.29"
wgpu = "23.0"
winit = "0.30"
```

- 理由: セキュリティ修正、バグ修正、パフォーマンス改善を享受

---

## 2. 実装スコープの仮決定

### 2.1 Phase 1 (MVP) スコープ

**目標**: 基本的なファイラー＋ビューアとして動作すること

| 機能 | Phase 1 | Phase 2 | Phase 3 |
|------|---------|---------|---------|
| ファイル一覧表示（グリッド/リスト） | ✅ | | |
| 画像ビューア（単一表示） | ✅ | | |
| キーボード/マウスナビゲーション | ✅ | | |
| SQLite メタデータ管理 | ✅ | | |
| サムネイルキャッシュ (RocksDB) | ✅ | | |
| 基本的な画像形式対応 (PNG/JPEG/WebP/BMP/GIF) | ✅ | | |
| 基本的なアーカイブ対応 (ZIP) | ✅ | | |
| 設定画面・キーバインド変更 | ✅ | | |
| 見開き表示 | | ✅ | |
| タグ管理 | | ✅ | |
| Susie Bridge (32bit) | | ✅ | |
| 画面分割・比較モード | | ✅ | |
| 高度なアーカイブ (RAR/7z/LZH) | | ✅ | |
| ファイル監視 (notify) | | ✅ | |
| Native Plugin API | | | ✅ |
| Lua スクリプト | | | ✅ |
| AI メタデータ解析 | | | ✅ |

---

### 2.2 コマンドIDの優先実装

**Phase 1 で実装するコマンド** (03_Input_UX.md より抜粋):

```
nav.prev_item, nav.next_item, nav.first_item, nav.last_item
nav.up_folder, nav.enter_folder
view.toggle_fullscreen, view.zoom_in, view.zoom_out, view.fit_to_window
view.original_size, view.rotate_left, view.rotate_right
file.delete, file.rename, file.copy_to, file.move_to
app.open_settings, app.quit
```

**Phase 2 以降に延期**:
```
nav.prev_folder, nav.next_folder (フォルダ履歴)
view.spread_* (見開き関連)
meta.* (タグ・評価関連)
file.external_* (外部アプリ連携)
```

---

## 3. 設計の曖昧点への対応

### 3.1 ファイル名サニタイズ戦略

**設計書の記述**: ルールを厳密に定義する必要がある

**仮決定**:
```
禁止文字: \ / : * ? " < > |
予約語: CON, PRN, AUX, NUL, COM1-9, LPT1-9

変換ルール（解凍時）:
1. 禁止文字 → 全角相当文字に置換
   : → ： (U+FF1A)
   * → ＊ (U+FF0A)
   ? → ？ (U+FF1F)
   etc.

2. 予約語 → アンダースコア付与
   CON → _CON

3. 設定で選択可能:
   - 全角置換 (デフォルト)
   - アンダースコア置換
   - %XX URLエスケープ
```

---

### 3.2 コンテキスト別コマンド挙動

**設計書の記述**: 物理フォルダ vs 論理ビュー（タグ検索）での挙動定義が必要

**仮決定**:

| コマンド | 物理フォルダ | タグ検索結果 |
|----------|-------------|-------------|
| `nav.next_item` | ファイルシステム順 | DB検索結果順 |
| `nav.up_folder` | 親ディレクトリ | 検索結果を閉じる |
| `file.delete` | ゴミ箱へ移動 | ゴミ箱へ移動 + DB更新 |
| `file.move_to` | 移動 + パス更新 | 移動 + パス更新 |

**実装**: `NavigationContext` enum で状態管理
```rust
pub enum NavigationContext {
    PhysicalFolder { path: UniversalPath },
    TagSearch { query: String, results: Vec<FileId> },
    Timeline { date_range: DateRange },
    Archive { archive_path: UniversalPath },
}
```

---

### 3.3 設定ファイル形式

**設計書の記述**: 明示なし

**仮決定**: **TOML** を採用
- 理由: Rust エコシステムの標準、人間が読みやすい
- 配置: `%APPDATA%\LightningFiler\config.toml`

```toml
[general]
language = "ja"
theme = "dark"

[viewer]
background_color = "#202020"
fit_mode = "fit"
interpolation = "lanczos3"

[keybindings]
next_item = ["Right", "l", "Space"]
prev_item = ["Left", "h", "Shift+Space"]
```

---

### 3.4 デフォルトキーバインド

**設計書の記述**: 一部のみ定義

**仮決定** (NeeView / QuickViewer 互換を意識):

| 操作 | デフォルトキー |
|------|--------------|
| 次の画像 | Right, L, Space, PageDown |
| 前の画像 | Left, H, Shift+Space, PageUp |
| 先頭へ | Home |
| 末尾へ | End |
| 上のフォルダ | Backspace, U |
| フォルダに入る | Enter, O |
| フルスクリーン | F11, F |
| ズームイン | +, Ctrl+Wheel Up |
| ズームアウト | -, Ctrl+Wheel Down |
| 等倍表示 | 1 |
| フィット表示 | 0 |
| 削除 | Delete |
| 終了 | Alt+F4, Q |

---

## 4. ビルド・配布の仮決定

### 4.1 ビルドターゲット

```
Primary:   x86_64-pc-windows-msvc (64bit Main)
Secondary: i686-pc-windows-msvc (32bit Susie Bridge) - Phase 2
```

### 4.2 配布形式

**仮決定**:
- **インストーラー**: WiX Toolset (.msi)
- **ポータブル版**: ZIP アーカイブ
- **自動更新**: 未実装（Phase 3 以降検討）

### 4.3 ディレクトリ構造（インストール後）

```
LightningFiler/
├── LightningFiler.exe      # 64bit メインバイナリ
├── susie_bridge.exe        # 32bit ブリッジ（Phase 2）
├── resources/
│   ├── locales/            # 言語ファイル
│   └── fonts/              # フォールバックフォント
├── plugins/
│   ├── native/             # .dll プラグイン
│   └── susie/              # .spi プラグイン
└── scripts/                # Luaスクリプト
```

---

## 5. 未決定事項（オリジナル設計者への質問）

以下の項目は実装を進めながら、必要に応じて決定する：

1. **アプリアイコン・ブランディング**: ロゴデザインは別途必要
2. **ライセンス**: MIT / Apache-2.0 / GPL の選択
3. **CI/CD**: GitHub Actions での自動ビルド設定
4. **テレメトリ**: クラッシュレポート送信の可否
5. **有償機能**: 将来的な収益化の計画

---

## 変更履歴

| 日付 | 変更内容 |
|------|---------|
| 2025-12-16 | 初版作成（Claude による仮決定） |

---

*本ドキュメントは実装の進行に応じて更新される。*
