# LightningFiler

**高速・高レスポンス・高カスタマイズ性** を誇る Windows 向け画像ファイル管理統合環境。

## 特徴

- **Zero Latency**: Rust + wgpu によるネイティブ実装。GCなし。
- **マルチエンコーディング対応**: 日本語、中国語、韓国語など多言語ファイル名を完全サポート
- **書庫透過閲覧**: ZIP/7z/RAR/LZH などをフォルダのように閲覧
- **Susie プラグイン互換**: 32bit レガシープラグインを IPC 経由で安全に使用
- **タグ管理**: Picasa 風のタグベース管理と高速検索
- **カスタマイズ**: 全コマンドに対応したキーバインド設定

## ビルド

### 必要環境

- Rust 1.75+
- Windows 10/11 (x64)
- Visual Studio Build Tools 2019/2022

### 64bit メインアプリケーション

```bash
cargo build --release
```

### 32bit Susie Bridge (オプション)

```bash
cargo build --release --target i686-pc-windows-msvc -p susie_host
```

## プロジェクト構造

```
LightningFiler/
├── Cargo.toml              # ワークスペース定義
├── crates/
│   ├── app_main/           # メインバイナリ (64bit)
│   ├── app_core/           # ドメインロジック・状態管理
│   ├── app_ui/             # egui + wgpu UI
│   ├── app_db/             # SQLite + RocksDB
│   ├── app_fs/             # VFS・パス処理
│   ├── app_log/            # ロギング・パニックフック
│   ├── susie_host/         # Susie Bridge (32bit)
│   └── ipc_proto/          # IPC プロトコル定義
├── resources/
│   └── locales/            # 多言語リソース (Fluent)
└── docs/                   # 設計書
```

## ライセンス

MIT License

## 謝辞

- 設計: wai55555
- 実装: Claude (Anthropic)
